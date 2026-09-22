use rodio::source::{SineWave, Source};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

const BEEP_HZ: f32 = 440.0;
const BEEP_VOLUME: f32 = 0.15;

/// Plays a continuous tone while enabled. Silent if no audio device is available.
pub struct Beeper {
    // The device sink must outlive the player, or playback stops.
    output: Option<(MixerDeviceSink, Player)>,
}

impl Beeper {
    pub fn new() -> Self {
        let output = match DeviceSinkBuilder::open_default_sink() {
            Ok(sink) => {
                let player = Player::connect_new(sink.mixer());
                player.append(SineWave::new(BEEP_HZ).amplify(BEEP_VOLUME).repeat_infinite());
                player.pause();
                Some((sink, player))
            }
            Err(e) => {
                eprintln!("Audio unavailable, running silently: {e}");
                None
            }
        };
        Beeper { output }
    }

    pub fn set_active(&self, active: bool) {
        if let Some((_, player)) = &self.output {
            if active {
                player.play();
            } else {
                player.pause();
            }
        }
    }
}
