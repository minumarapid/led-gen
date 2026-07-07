use std::fs;
use std::path::Path;

use clap::Args;
use led_gen_core::{parse_off_light_color, LedConfig, LedError, LedShape};

#[derive(Args, Clone, Debug, Default)]
pub struct CliLedConfig {
    /// Canvas border size in pixels
    #[arg(long)]
    pub border: Option<u32>,

    /// LED size in pixels
    #[arg(long)]
    pub led_size: Option<u32>,

    /// Gap between LEDs in pixels
    #[arg(long)]
    pub led_gap: Option<u32>,

    /// LED shape
    #[arg(long, value_enum)]
    pub led_shape: Option<LedShape>,

    /// LED color exposure multiplier
    #[arg(long)]
    pub led_exposure: Option<f32>,

    /// Disable glow
    #[arg(long = "no-glow", action = clap::ArgAction::SetTrue)]
    pub no_glow: bool,

    /// Glow blur radius
    #[arg(long)]
    pub glow_range: Option<f32>,

    /// Glow strength multiplier
    #[arg(long)]
    pub glow_strength: Option<f32>,

    /// Glow color exposure multiplier
    #[arg(long)]
    pub glow_exposure: Option<f32>,

    /// Minimum LED color as R,G,B
    #[arg(long, value_parser = parse_off_light_color)]
    pub off_light_color: Option<[u8; 3]>,

    /// Canvas background color as R,G,B
    #[arg(long, value_parser = parse_off_light_color)]
    pub canvas_background: Option<[u8; 3]>,
}

impl CliLedConfig {
    pub fn apply_to(self, config: &mut LedConfig) {
        if let Some(value) = self.border {
            config.border = value;
        }
        if let Some(value) = self.led_size {
            config.led_size = value;
        }
        if let Some(value) = self.led_gap {
            config.led_gap = value;
        }
        if let Some(value) = self.led_shape {
            config.led_shape = value;
        }
        if let Some(value) = self.led_exposure {
            config.led_exposure = value;
        }
        if self.no_glow {
            config.enable_glow = false;
        }
        if let Some(value) = self.glow_range {
            config.glow_range = value;
        }
        if let Some(value) = self.glow_strength {
            config.glow_strength = value;
        }
        if let Some(value) = self.glow_exposure {
            config.glow_exposure = value;
        }
        if let Some(value) = self.off_light_color {
            config.off_light_color = value;
        }
        if let Some(value) = self.canvas_background {
            config.canvas_background = value;
        }
    }
}

pub fn load_config(path: &Path) -> Result<LedConfig, LedError> {
    let text = fs::read_to_string(path).map_err(|e| {
        LedError::InvalidConfiguration(format!(
            "Unable to read the config file {} ({})",
            path.display(),
            e
        ))
    })?;

    toml::from_str(&text).map_err(|e| {
        LedError::InvalidConfiguration(format!(
            "Unable to parse the config file {} ({})",
            path.display(),
            e
        ))
    })
}
