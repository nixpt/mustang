//! Mustang configuration
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.

/// Mustang processing mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MustangMode {
    /// CPU-only processing (fallback)
    CpuOnly,
    /// GPU-accelerated processing
    GpuAccelerated,
    /// Hybrid mode (GPU with CPU fallback)
    Hybrid,
}

impl Default for MustangMode {
    fn default() -> Self {
        MustangMode::GpuAccelerated
    }
}

/// Configuration for Mustang compositor
#[derive(Debug, Clone)]
pub struct MustangConfig {
    /// Processing mode
    pub mode: MustangMode,
    /// Enable effect caching
    pub enable_caching: bool,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Enable debug visualization
    pub enable_debug: bool,
}

impl Default for MustangConfig {
    fn default() -> Self {
        Self {
            mode: MustangMode::GpuAccelerated,
            enable_caching: true,
            max_cache_size: 1000,
            enable_debug: false,
        }
    }
}

impl MustangConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set processing mode
    pub fn mode(mut self, mode: MustangMode) -> Self {
        self.mode = mode;
        self
    }

    /// Enable/disable caching
    pub fn enable_caching(mut self, enable: bool) -> Self {
        self.enable_caching = enable;
        self
    }

    /// Set maximum cache size
    pub fn max_cache_size(mut self, size: usize) -> Self {
        self.max_cache_size = size;
        self
    }

    /// Enable/disable debug visualization
    pub fn enable_debug(mut self, enable: bool) -> Self {
        self.enable_debug = enable;
        self
    }

    /// Create CPU-only configuration
    pub fn cpu_only() -> Self {
        Self::default().mode(MustangMode::CpuOnly)
    }

    /// Create GPU-accelerated configuration
    pub fn gpu_accelerated() -> Self {
        Self::default().mode(MustangMode::GpuAccelerated)
    }

    /// Create hybrid configuration
    pub fn hybrid() -> Self {
        Self::default().mode(MustangMode::Hybrid)
    }
}
