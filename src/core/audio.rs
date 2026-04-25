//! 音频数据相关类型

use serde::{Deserialize, Serialize};

/// 音频数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// 采样率
    pub sample_rate: u32,

    /// 声道数
    pub channels: u16,

    /// 音频采样数据
    pub samples: Vec<i16>,
}

impl AudioData {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
            samples: Vec::new(),
        }
    }
}
