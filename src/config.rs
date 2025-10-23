use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub timers: Vec<Timer>,
    pub input_tracking: Option<InputTracking>,
    #[serde(with = "duration_format")]
    pub timeout_reset: Duration,
}

impl Config {
    pub fn load_or_create(path: PathBuf) -> Result<Self> {
        if path.exists() {
            let config_str = fs::read_to_string(&path).context("failed to read config file")?;
            serde_yaml::from_str::<Self>(&config_str).context("failed to parse config file")
        } else {
            let default_config = Config::default();
            let config_str = serde_yaml::to_string(&default_config).unwrap();
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).context("failed to create config directory")?;
            }
            fs::write(&path, &config_str).context("failed to write config file")?;
            println!("Created config file at '{}'", path.display());
            Ok(default_config)
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub descriptions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Sound {
    pub path: PathBuf,
    #[serde(with = "duration_format")]
    pub duration: Duration,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InputTracking {
    #[serde(with = "duration_format")]
    pub inactivity_pause: Duration,
    #[serde(with = "duration_format")]
    pub inactivity_reset: Duration,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Timer {
    #[serde(with = "duration_format")]
    pub interval: Duration,
    #[serde(
        default,
        with = "duration_format_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout: Option<Duration>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub weight: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub decline: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<Notification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<Sound>,
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
                        title: String::from("Small break"),
                        descriptions: vec![
                            "Time to get a cup of cofee.".to_string(),
                            "Time to get away from your desk.".to_string(),
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Timer {
                    interval: Duration::from_secs(60 * 20 * 3),
                    notification: Some(Notification {
                        title: String::from("Big break"),
                        descriptions: vec![
                            "Time to relax. You've been using the computer for {} minutes."
                                .to_string(),
                        ],
                        ..Default::default()
                    }),
                    weight: 1,
                    decline: 0.6,
                    ..Default::default()
                },
            ],
            input_tracking: Some(InputTracking {
                inactivity_pause: Duration::from_secs(30),
                inactivity_reset: Duration::from_secs(60 * 2),
            }),
            timeout_reset: Duration::from_secs(60 * 3),
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
        let mins = total_secs / 60;
        let secs = total_secs - mins * 60;
        serializer.serialize_str(&(format!("{:02}:{:02}", mins, secs)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let center = str
            .find(":")
            .ok_or(Error::custom("missing ':' splitter on duration"))?;
        let mins = &str[..center]
            .parse::<u64>()
            .map_err(|e| Error::custom(format!("failed to parse left integer: {}", e)))?;
        let secs = &str[center + 1..]
            .parse::<u64>()
            .map_err(|e| Error::custom(format!("failed to parse right integer: {}", e)))?;

        Ok(Duration::from_secs(mins * 60 + secs))
    }
}

mod duration_format_option {
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
