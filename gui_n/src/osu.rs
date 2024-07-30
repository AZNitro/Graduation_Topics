use crate::read_config;
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use reqwest::Client;
use serde::Deserialize;
use regex::Regex;
use image::load_from_memory;
use tokio::sync::mpsc::Sender;
use tokio::try_join;
use std::sync::Arc;
use egui::TextureHandle;
use egui::ColorImage;
use thiserror::Error;

#[derive(Debug, Deserialize, Clone)]
pub struct Covers {
    pub cover: Option<String>,
    pub cover_2x: Option<String>,
    pub card: Option<String>,
    pub card_2x: Option<String>,
    pub list: Option<String>,
    pub list_2x: Option<String>,
    pub slimcover: Option<String>,
    pub slimcover_2x: Option<String>,
}
#[derive(Debug, Deserialize, Clone)] // �K�[ Clone
pub struct Beatmapset {
    pub beatmaps: Vec<Beatmap>,
    pub id: i32,
    pub artist: String,
    pub title: String,
    pub creator: String,
    pub covers: Covers,
}
#[derive(Deserialize)]
pub struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    beatmapsets: Vec<Beatmapset>,
}
#[derive(Debug, Deserialize, Clone)]
pub struct Beatmap {
    pub difficulty_rating: f32,
    pub id: i32,
    pub mode: String,
    pub status: String,
    pub total_length: i32,
    pub user_id: i32,
    pub version: String,
}
pub struct BeatmapInfo {
    pub title: String,
    pub artist: String,
    pub creator: String,
    pub beatmaps: Vec<String>,
}

#[derive(Error, Debug)]
pub enum OsuError {
    #[error("�ШD���~: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("JSON �ѪR���~: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("�t�m���~: {0}")]
    ConfigError(String),
    #[error("��L���~: {0}")]
    Other(String),
}

pub async fn get_beatmapsets(
    client: &Client,
    access_token: &str,
    song_name: &str,
    debug_mode: bool,
) -> Result<Vec<Beatmapset>, OsuError> {
    let response = client
        .get("https://osu.ppy.sh/api/v2/beatmapsets/search")
        .query(&[("query", song_name)])
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(OsuError::RequestError)?;

    let response_text = response.text().await
        .map_err(OsuError::RequestError)?;

    if debug_mode {
        info!("Osu API �^�� JSON: {}", response_text);
    }

    let search_response: SearchResponse = serde_json::from_str(&response_text)
        .map_err(OsuError::JsonError)?;

    Ok(search_response.beatmapsets)
}

pub async fn get_beatmapset_by_id(
    client: &Client,
    access_token: &str,
    beatmapset_id: &str,
    debug_mode: bool,
) -> Result<Beatmapset, OsuError> {
    let url = format!("https://osu.ppy.sh/api/v2/beatmapsets/{}", beatmapset_id);

    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(OsuError::RequestError)?;

    let response_text = response.text().await
        .map_err(OsuError::RequestError)?;

    if debug_mode {
        info!("Osu API �^�� JSON: {}", response_text);
    }

    let beatmapset: Beatmapset = serde_json::from_str(&response_text)
        .map_err(OsuError::JsonError)?;

    Ok(beatmapset)
}

pub async fn get_beatmapset_details(
    client: &Client,
    access_token: &str,
    beatmapset_id: &str,
    debug_mode: bool,
) -> Result<(String, String), OsuError> {
    let url = format!("https://osu.ppy.sh/api/v2/beatmapsets/{}", beatmapset_id);

    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(OsuError::RequestError)?;

    let beatmapset: serde_json::Value = response
        .json()
        .await
        .map_err(OsuError::RequestError)?;

    if debug_mode {
        println!("Beatmapset details: {:?}", beatmapset);
    }

    let (artist, title) = try_join!(
        async { Ok::<_, OsuError>(beatmapset["artist"].as_str().unwrap_or("Unknown Artist").to_string()) },
        async { Ok::<_, OsuError>(beatmapset["title"].as_str().unwrap_or("Unknown Title").to_string()) }
    )?;

    Ok((artist, title))
}
pub async fn get_osu_token(client: &Client, debug_mode: bool) -> Result<String, OsuError> {
    if debug_mode {
        debug!("�}�l��� Osu token");
    }

    let config = read_config(debug_mode).map_err(|e| {
        error!("Ū���t�m���ɥX��: {}", e);
        OsuError::ConfigError(format!("Error reading config: {}", e))
    })?;

    let client_id = &config.osu.client_id;
    let client_secret = &config.osu.client_secret;

    if debug_mode {
        debug!("���\Ū�� Osu client_id �M client_secret");
    }

    let url = "https://osu.ppy.sh/oauth/token";
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", &"client_credentials".to_string()),
        ("scope", &"public".to_string()),
    ];

    if debug_mode {
        debug!("�ǳƵo�e Osu token �ШD");
    }

    let response = client
        .post(url)
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            error!("�o�e Osu token �ШD�ɥX��: {}", e);
            OsuError::RequestError(e)
        })?;

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| {
            error!("�ѪR Osu token �^���ɥX��: {}", e);
            OsuError::RequestError(e)
        })?;

    if debug_mode {
        debug!("���\��� Osu token");
    }

    Ok(token_response.access_token)
}

impl Beatmapset {
    pub fn format_info(&self) -> BeatmapInfo {
        let beatmaps = self.beatmaps.iter().map(|b| b.format_info()).collect();
        BeatmapInfo {
            title: self.title.clone(),
            artist: self.artist.clone(),
            creator: self.creator.clone(),
            beatmaps,
        }
    }
}

impl Beatmap {
    pub fn format_info(&self) -> String {
        format!(
            "Difficulty: {:.2} | Mode: {} | Status: {}\nLength: {} min {}s | Version: {}",
            self.difficulty_rating,
            self.mode,
            self.status,
            self.total_length / 60,
            self.total_length % 60,
            self.version
        )
    }
}

pub fn print_beatmap_info_gui(beatmapset: &Beatmapset) -> BeatmapInfo {
    beatmapset.format_info()
}
pub fn parse_osu_url(url: &str) -> Option<(String, Option<String>)> {
    let beatmapset_regex =
        Regex::new(r"https://osu\.ppy\.sh/beatmapsets/(\d+)(?:#(\w+)/(\d+))?$").unwrap();

    if let Some(captures) = beatmapset_regex.captures(url) {
        let beatmapset_id = captures.get(1).unwrap().as_str().to_string();
        let beatmap_id = captures.get(3).map(|m| m.as_str().to_string());
        Some((beatmapset_id, beatmap_id))
    } else {
        None
    }
}
pub async fn load_osu_covers(
    urls: Vec<String>,
    ctx: egui::Context,
    sender: Sender<(usize, Arc<TextureHandle>, (f32, f32))>,
) -> Result<(), OsuError> {
    let client = Client::new();
    let mut errors = Vec::new();

    for (index, url) in urls.into_iter().enumerate() {
        debug!("���b���J�ʭ��AURL: {}", url);
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(bytes) => match load_from_memory(&bytes) {
                            Ok(image) => {
                                debug!("���\�q�O������J�Ϥ��AURL: {}", url);
                                let color_image = ColorImage::from_rgba_unmultiplied(
                                    [image.width() as usize, image.height() as usize],
                                    &image.to_rgba8(),
                                );
                                let texture = ctx.load_texture(
                                    format!("cover_{}", index),
                                    color_image,
                                    Default::default(),
                                );
                                let texture = Arc::new(texture);
                                let size = (image.width() as f32, image.height() as f32);
                                if let Err(e) = sender.send((index, texture, size)).await {
                                    error!("�o�e���z���ѡAURL: {}, ���~: {:?}", url, e);
                                    errors.push(format!("�o�e���z���ѡAURL: {}, ���~: {:?}", url, e));
                                } else {
                                    debug!("���\�o�e���z�AURL: {}", url);
                                }
                            }
                            Err(e) => {
                                error!("�q�O������J�Ϥ����ѡAURL: {}, ���~: {:?}", url, e);
                                errors.push(format!("�q�O������J�Ϥ����ѡAURL: {}, ���~: {:?}", url, e));
                            }
                        },
                        Err(e) => {
                            error!("�q�^������줸�ե��ѡAURL: {}, ���~: {:?}", url, e);
                            errors.push(format!("�q�^������줸�ե��ѡAURL: {}, ���~: {:?}", url, e));
                        }
                    }
                } else {
                    error!("���J�ʭ����ѡAURL: {}, ���A�X: {}", url, response.status());
                    errors.push(format!("���J�ʭ����ѡAURL: {}, ���A�X: {}", url, response.status()));
                }
            }
            Err(e) => {
                error!("�o�e�ШD���ѡAURL: {}, ���~: {:?}", url, e);
                errors.push(format!("�o�e�ШD���ѡAURL: {}, ���~: {:?}", url, e));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(OsuError::Other(errors.join("\n")))
    }
}
