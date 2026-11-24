//! Process-wide admission and lifecycle policy for on-demand Ollama calls.
//! One in-flight request is conservative across shared/bundled runtimes.
//! Never evict models owned by another client or kill an Ollama process.
use std::{sync::OnceLock,time::Duration};
use tokio::sync::{Semaphore,SemaphorePermit};
use serde_json::{Value,json};
const MIB:u64=1024*1024;
const FLOOR_MIB:u64=2048;
const WARM_MIB:u64=6144;
const QUEUE_TIMEOUT:Duration=Duration::from_secs(20);
fn gate()->&'static Semaphore{static G:OnceLock<Semaphore>=OnceLock::new();G.get_or_init(||Semaphore::new(1))}
#[cfg(windows)]
pub(super) fn available_mib()->Option<u64>{
    #[repr(C)]
    struct MemoryStatus {length:u32,load:u32,total_phys:u64,avail_phys:u64,total_page:u64,avail_page:u64,total_virtual:u64,avail_virtual:u64,avail_extended:u64}
    #[link(name="kernel32")]
    extern "system" {fn GlobalMemoryStatusEx(status:*mut MemoryStatus)->i32;}
    let mut s=MemoryStatus{length:std::mem::size_of::<MemoryStatus>() as u32,load:0,total_phys:0,avail_phys:0,total_page:0,avail_page:0,total_virtual:0,avail_virtual:0,avail_extended:0};
    if unsafe{GlobalMemoryStatusEx(&mut s)}!=0{Some(s.avail_phys/MIB)}else{None}
}
#[cfg(not(windows))]
pub(super) fn available_mib()->Option<u64>{
    let data=std::fs::read_to_string("/proc/meminfo").ok()?;
    data.lines().find_map(|line|line.strip_prefix("MemAvailable:").and_then(|s|s.split_whitespace().next()?.parse::<u64>().ok()).map(|kb|kb/1024))
}
fn admitted(memory:Option<u64>)->bool{memory.map_or(true,|m|m>=FLOOR_MIB)}
fn retention(memory:Option<u64>)->Value{if memory.is_some_and(|m|m>=WARM_MIB){json!("20s")}else{json!(0)}}
pub struct Lease{_permit:SemaphorePermit<'static>,keep_alive:Value}
impl Lease{pub fn keep_alive(&self)->Value{self.keep_alive.clone()}}
/// Cancellation of the caller drops the queue/lease automatically. A dropped
/// HTTP future is not proof that the server has terminated its computation.
pub async fn acquire()->anyhow::Result<Lease>{
    let deadline=tokio::time::Instant::now()+QUEUE_TIMEOUT;
    let permit=tokio::time::timeout_at(deadline,gate().acquire()).await
        .map_err(|_|anyhow::anyhow!("Local inference queue is busy; retry after the current request finishes"))??;
    loop{
        let free=available_mib();
        if admitted(free){return Ok(Lease{_permit:permit,keep_alive:retention(free)});}
        if tokio::time::Instant::now()>=deadline{anyhow::bail!("Local inference deferred: less than 2 GiB of available RAM");}
        tokio::time::sleep_until((tokio::time::Instant::now()+Duration::from_millis(250)).min(deadline)).await;
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn memory_policy_has_explicit_boundaries(){
        assert!(!admitted(Some(2047)));assert!(admitted(Some(2048)));assert!(admitted(None));
        assert_eq!(retention(Some(3500)),json!(0));assert_eq!(retention(None),json!(0));
        assert_eq!(retention(Some(6143)),json!(0));assert_eq!(retention(Some(6144)),json!("20s"));
    }
    #[tokio::test]async fn cancelled_waiter_does_not_leak_capacity(){
        let semaphore=Semaphore::new(1);let first=semaphore.acquire().await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(5),semaphore.acquire()).await.is_err());
        drop(first);assert!(semaphore.try_acquire().is_ok());
    }
}
