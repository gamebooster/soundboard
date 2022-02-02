use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use serde::Deserialize;
use serde::Serialize;
use tokio::runtime::{Builder, Runtime};
use tonic::{
    codegen::InterceptedService,
    metadata::MetadataValue,
    service::Interceptor,
    transport::{Certificate, Channel, ClientTlsConfig},
    Request, Status,
};

mod google_cloud_texttospeech_v1 {
    tonic::include_proto!("google.cloud.texttospeech.v1");
}

use google_cloud_texttospeech_v1::synthesis_input::InputSource;
use google_cloud_texttospeech_v1::text_to_speech_client::TextToSpeechClient;
use google_cloud_texttospeech_v1::AudioConfig;
use google_cloud_texttospeech_v1::AudioEncoding;
use google_cloud_texttospeech_v1::SsmlVoiceGender;
use google_cloud_texttospeech_v1::SynthesisInput;
use google_cloud_texttospeech_v1::SynthesizeSpeechRequest;
use google_cloud_texttospeech_v1::SynthesizeSpeechResponse;
use google_cloud_texttospeech_v1::VoiceSelectionParams;

const ENDPOINT: &str = "https://texttospeech.googleapis.com";

pub struct TTSClient {
    client: TextToSpeechClient<InterceptedService<Channel, KeyInterceptor>>,
    rt: Runtime,
}

#[derive(Clone, Default, Debug)]
pub struct SynthesisOptions {
    pub speaking_rate: Option<f64>,
    pub pitch: Option<f64>,
    pub volume_gain_db: Option<f64>,
    pub voice_name: Option<String>,
    pub voice_gender: Option<SsmlVoiceGender>,
}

impl PartialEq for SynthesisOptions {
    fn eq(&self, other: &SynthesisOptions) -> bool {
        ((self.pitch.unwrap_or_default() * 10.0) as usize)
            == ((other.pitch.unwrap_or_default() * 10.0) as usize)
            && ((self.speaking_rate.unwrap_or_default() * 10.0) as usize)
                == ((other.speaking_rate.unwrap_or_default() * 10.0) as usize)
            && ((self.volume_gain_db.unwrap_or_default() * 10.0) as usize)
                == ((other.volume_gain_db.unwrap_or_default() * 10.0) as usize)
            && self.voice_name == other.voice_name
            && self.voice_gender == other.voice_gender
    }
}

impl std::hash::Hash for SynthesisOptions {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.voice_name.hash(state);
        self.voice_gender.hash(state);
        ((self.pitch.unwrap_or_default() * 10.0) as usize).hash(state);
        ((self.volume_gain_db.unwrap_or_default() * 10.0) as usize).hash(state);
        ((self.speaking_rate.unwrap_or_default() * 10.0) as usize).hash(state);
    }
}

struct KeyInterceptor;

impl Interceptor for KeyInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        req.metadata_mut().insert(
            "x-goog-api-key",
            MetadataValue::from_str("AIzaSyDP7orD6c32-E2TvzyEXbAimY1Fwbif_6c").unwrap(),
        );
        Ok(req)
    }
}

impl TTSClient {
    pub fn connect() -> Result<Self, tonic::transport::Error> {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        let tls_config = ClientTlsConfig::new().domain_name("texttospeech.googleapis.com");

        let channel = rt.block_on(
            Channel::from_static(ENDPOINT)
                .tls_config(tls_config)?
                .connect(),
        )?;

        let client = TextToSpeechClient::with_interceptor(channel, KeyInterceptor);

        Ok(Self { rt, client })
    }

    fn synthesize_speech_request(
        &mut self,
        request: impl tonic::IntoRequest<SynthesizeSpeechRequest>,
    ) -> Result<tonic::Response<SynthesizeSpeechResponse>, tonic::Status> {
        self.rt.block_on(self.client.synthesize_speech(request))
    }

    pub fn synthesize_speech(
        &mut self,
        ssml: &str,
        language_code: &str,
        options: Option<SynthesisOptions>,
    ) -> Result<Vec<u8>> {
        let mut request = SynthesizeSpeechRequest::default();
        let options = options.unwrap_or_default();

        let mut input = SynthesisInput::default();
        input.input_source = Some(InputSource::Ssml(ssml.to_owned()));
        request.input = Some(input);

        let mut voice_selection = VoiceSelectionParams::default();
        voice_selection.language_code = language_code.to_owned();
        if let Some(voice_name) = options.voice_name {
            voice_selection.name = voice_name;
        }
        if let Some(voice_gender) = options.voice_gender {
            voice_selection.ssml_gender = voice_gender as i32;
        }
        request.voice = Some(voice_selection);

        let mut audio_config = AudioConfig::default();
        audio_config.audio_encoding = AudioEncoding::OggOpus as i32;
        audio_config.pitch = options.pitch.unwrap_or_default();
        audio_config.speaking_rate = options.speaking_rate.unwrap_or_default();
        audio_config.volume_gain_db = options.volume_gain_db.unwrap_or_default();
        request.audio_config = Some(audio_config);
        let response = self.synthesize_speech_request(request)?.into_inner();
        Ok(response.audio_content)
    }
}
