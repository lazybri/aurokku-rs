//! 内存版 UsbTransport：用脚本化的响应字节流模拟设备端点。
//!
//! 存在的意义：把“协议核心 + 传输抽象”在没有 USB 硬件的情况下跑通 E2E 单测，
//! 并且可以按任意 chunk 切分响应来模拟真实端点的分片行为。

use alloc::collections::VecDeque;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::{AuRokkuError, AuRokkuResult, UsbTransport};

/// 记录写入、按脚本回放读取的 mock 传输。
#[derive(Debug, Clone)]
pub struct MockTransport {
    /// 每次 write 的完整帧，按顺序记录（用于断言协议顺序）。
    pub writes: Vec<Vec<u8>>,
    /// 尚未交付给核心的响应字节。
    scripted: VecDeque<u8>,
    /// 单次 read 最多返回多少字节（模拟端点分片；usize::MAX 表示一次给完）。
    chunk: usize,
}

impl MockTransport {
    /// responses 按“一帧一个切片”给出，内部按顺序拼成一条字节流。
    pub fn new(responses: &[&[u8]]) -> Self {
        let mut scripted = VecDeque::new();
        for response in responses {
            scripted.extend(response.iter().copied());
        }
        Self {
            writes: Vec::new(),
            scripted,
            chunk: usize::MAX,
        }
    }

    /// 限制单次 read 的返回长度，用于验证核心的缓冲/累积逻辑。
    pub fn with_chunk(mut self, chunk: usize) -> Self {
        self.chunk = chunk.max(1);
        self
    }

    /// 剩余未交付的响应字节数。
    pub fn remaining(&self) -> usize {
        self.scripted.len()
    }
}

impl UsbTransport for MockTransport {
    fn write(&mut self, data: &[u8]) -> AuRokkuResult<()> {
        self.writes.push(data.to_vec());
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> AuRokkuResult<usize> {
        if self.scripted.is_empty() {
            return Err(AuRokkuError::Timeout(
                "mock transport: scripted responses exhausted".to_string(),
            ));
        }
        let n = buf.len().min(self.chunk).min(self.scripted.len());
        let taken: Vec<u8> = self.scripted.drain(..n).collect();
        buf[..n].copy_from_slice(&taken);
        Ok(n)
    }
}
