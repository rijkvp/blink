use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub timers: Vec<Timer>,
    pub input_tracking: Option<InputTracking>,
}

impl Config {
    pub fn load_or_create(path: PathBuf) -> Result<Self> {
        if path.exists() {
            let config_str = fs::read_to_string(&path).context("failed to read config file")?;
            serde_yaml_ng::from_str::<Self>(&config_str).context("failed to parse config file")
        } else {
            let default_config = Config::default();
            let config_str = serde_yaml_ng::to_string(&default_config).unwrap();
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).context("failed to create config directory")?;
            }
            fs::write(&path, &config_str).context("failed to write config file")?;
            log::info!("Created default config at '{}'", path.display());
            Ok(default_config)
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub title: String,
    pub descriptions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InputTracking {
    #[serde(with = "duration_format")]
    pub pause_after: Duration,
    #[serde(with = "duration_format")]
    pub reset_after: Duration,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Timer {
    #[serde(with = "duration_format")]
    pub interval: Duration,
    #[serde(
        default,
        with = "duration_format_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub initial_delay: Option<Duration>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub decline: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<Notification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timers: vec![
                Timer {
                    interval: Duration::from_secs(60 * 20),
                    notification: Some(Notification {
                        title: String::from("Microbreak"),
                        descriptions: vec![
                            "Look away from your screen for 20 seconds.".to_string(),
                            "Roll your shoulders and stretch your neck.".to_string(),
                            "Stand up and change your posture.".to_string(),
                        ],
                        timeout: Some(10),
                    }),
                    ..Default::default()
                },
                Timer {
                    interval: Duration::from_secs(60 * 60),
                    notification: Some(Notification {
                        title: String::from("Take a break!"),
                        descriptions: vec![
                            "You've been at your screen for {}. Time for a short walk or a stretch!"
                                .to_string(),
                        ],
                        timeout: Some(0), // will never time out
                    }),
                    decline: 0.5,
                    ..Default::default()
                },
            ],
            input_tracking: None,
        }
    }
}

mod duration_format {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let total_secs = duration.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if hours > 0 {
            serializer.serialize_str(&format!("{:02}:{:02}:{:02}", hours, mins, secs))
        } else {
            serializer.serialize_str(&format!("{:02}:{:02}", mins, secs))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let parts: Vec<&str> = str.split(':').collect();

        match parts.len() {
            2 => {
                // mm:ss format
                let mins = parts[0]
                    .parse::<u64>()
                    .map_err(|e| Error::custom(format!("failed to parse minutes: {}", e)))?;
                let secs = parts[1]
                    .parse::<u64>()
                    .map_err(|e| Error::custom(format!("failed to parse seconds: {}", e)))?;

                if secs > 59 {
                    return Err(Error::custom("seconds must be in range 0-59"));
                }

                Ok(Duration::from_secs(mins * 60 + secs))
            }
            3 => {
                // hh:mm:ss format
                let hours = parts[0]
                    .parse::<u64>()
                    .map_err(|e| Error::custom(format!("failed to parse hours: {}", e)))?;
                let mins = parts[1]
                    .parse::<u64>()
                    .map_err(|e| Error::custom(format!("failed to parse minutes: {}", e)))?;
                let secs = parts[2]
                    .parse::<u64>()
                    .map_err(|e| Error::custom(format!("failed to parse seconds: {}", e)))?;

                if mins > 59 {
                    return Err(Error::custom("minutes must be in range 0-59"));
                }
                if secs > 59 {
                    return Err(Error::custom("seconds must be in range 0-59"));
                }

                Ok(Duration::from_secs(hours * 3600 + mins * 60 + secs))
            }
            _ => Err(Error::custom(
                "duration must be in format 'mm:ss' or 'hh:mm:ss'",
            )),
        }
    }
}

mod duration_format_opt {
    use super::duration_format;
    use serde::{Deserializer, Serializer, de::Error};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(dur) => duration_format::serialize(dur, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match duration_format::deserialize(deserializer) {
            Ok(dur) => Ok(Some(dur)),
            Err(err) => Err(Error::custom(err)),
        }
    }
}
