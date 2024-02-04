#![feature(if_let_guard)]

use slack_morphism::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

const ER_SOUND: &str = "ɝ";
const DELIMITER: char = '\0';

#[derive(Debug, Clone, Eq, PartialEq)]
enum PartOfSpeech {
  Noun,
  Plural,
  NounPhrase,
  VerbUsuParticiple,
  VerbTransitive,
  VerbIntransitive,
  Adjective,
  Adverb,
  Conjunction,
  Preposition,
  Interjection,
  Pronoun,
  DefiniteArticle,
  IndefiniteArticle,
  Nominative,
  E,
}

impl From<char> for PartOfSpeech {
  fn from(character: char) -> Self {
    match character {
      'N' => Self::Noun,
      'p' => Self::Plural,
      'h' => Self::NounPhrase,
      'V' => Self::VerbUsuParticiple,
      't' => Self::VerbTransitive,
      'i' => Self::VerbIntransitive,
      'A' => Self::Adjective,
      'v' => Self::Adverb,
      'C' => Self::Conjunction,
      'P' => Self::Preposition,
      '!' => Self::Interjection,
      'r' => Self::Pronoun,
      'D' => Self::DefiniteArticle,
      'I' => Self::IndefiniteArticle,
      'o' => Self::Nominative,
      'e' => Self::E,
      unknown => {
        unreachable!("Chom: {unknown:?}")
      }
    }
  }
}

type Word = String;
type Pronunciation = &'static str;

lazy_static::lazy_static! {
  static ref SOUNDS_TO_TEXT: HashMap<Pronunciation, Vec<Word>> = {
    let mut dictionary: HashMap<Pronunciation, Vec<Word>> = HashMap::new();
    for line in include_str!("dictionary/text_to_sounds.txt").lines() {
      if let Some((word, pronunciation)) = line.split_once('\t') {
        let word = word.to_lowercase();
        let pronunciation = pronunciation.strip_suffix('/').unwrap().strip_prefix('/').unwrap();

        if let Some(values) = dictionary.get_mut(pronunciation) {
          values.push(word);
        } else {
          dictionary.insert(pronunciation, vec![word]);
        }
      }
    }
    dictionary
  };
  static ref TEXT_TO_SOUNDS: HashMap<Word, Vec<Pronunciation>> = {
    let mut dictionary: HashMap<Word, Vec<Pronunciation>> = HashMap::new();
    for line in include_str!("dictionary/text_to_sounds.txt").lines() {
      if let Some((word, pronunciation)) = line.split_once('\t') {
        let word = word.to_lowercase();
        let pronunciation = pronunciation.strip_suffix('/').unwrap().strip_prefix('/').unwrap();

        if let Some(values) = dictionary.get_mut(&word) {
          values.push(pronunciation);
        } else {
          dictionary.insert(word.to_string(), vec![pronunciation]);
        }
      }
    }
    dictionary
  };
  static ref PARTS_OF_SPEECH: HashMap<&'static str, Vec<PartOfSpeech>> = {
    let mut dictionary = HashMap::new();
    for entry in include_str!("dictionary/pos.txt").split('\n') {
      let (word, pos) = entry.split_once(DELIMITER).unwrap();
      dictionary.insert(word, pos.chars().map(PartOfSpeech::from).collect::<Vec<_>>());
    }
    dictionary
  };
}

fn get_er_word(word: String) -> Option<String> {
  if let Some(pronunciations) = TEXT_TO_SOUNDS.get(&word.to_string()) {
    for pronunciation in pronunciations {
      if let Some(er_less_sound) = pronunciation.strip_suffix(ER_SOUND) {
        println!("Found an er-less sound: {er_less_sound}");
        if let Some(words) = SOUNDS_TO_TEXT.get(er_less_sound) {
          println!("Words are: {words:?}");
          for er_less_word in words {
            let er_less_word: &String = &er_less_word.to_string();
            println!(
              "We found a part of speech!!! {:?}",
              PARTS_OF_SPEECH.get(er_less_word.as_str())
            );
            if let Some(pos) = PARTS_OF_SPEECH.get(er_less_word.as_str()) {
              for pos in pos {
                if *pos == PartOfSpeech::VerbTransitive || *pos == PartOfSpeech::VerbUsuParticiple {
                  return Some(er_less_word.to_string());
                }
              }
            }
          }
        }
      }
    }
  }
  None
}

async fn on_push_event(
  event: SlackPushEventCallback,
  client: Arc<SlackHyperClient>,
  _states: SlackClientEventsUserState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  println!("Push event: {:#?}", event);
  let token_value: SlackApiTokenValue = config_env_var("SLACK_BOT_TOKEN")?.into();
  let token: SlackApiToken = SlackApiToken::new(token_value);

  let session = client.open_session(&token);

  match event.event {
    SlackEventCallbackBody::Message(message)
      if let Some(text) = message
        .content
        .as_ref()
        .and_then(|content| content.text.clone()) =>
    {
      println!("{text}");
      println!("{message:?}");
      for word in text.to_lowercase().split_whitespace() {
        if let Some(er_less_word) = get_er_word(word.to_string()) {
          println!("Word discovered! {er_less_word}");
          session
            .chat_post_message(
              &SlackApiChatPostMessageRequest::new(message.origin.channel.unwrap(), {
                let mut content = SlackMessageContent::new();
                content.text(format!("{er_less_word} 'er?! I hardly know her!"));
                content
              })
              .with_thread_ts(message.origin.thread_ts.unwrap_or(message.origin.ts)),
            )
            .await?;
          break;
        }
      }
      Ok(())
    }
    _ => Ok(()),
  }
}

fn test_error_handler(
  err: Box<dyn std::error::Error + Send + Sync>,
  _client: Arc<SlackHyperClient>,
  _states: SlackClientEventsUserState,
) -> HttpStatusCode {
  println!("{:#?}", err);

  // This return value should be OK if we want to return successful ack to the Slack server using Web-sockets
  // https://api.slack.com/apis/connections/socket-implement#acknowledge
  // so that Slack knows whether to retry
  HttpStatusCode::OK
}

async fn test_client_with_socket_mode() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let client = Arc::new(SlackClient::new(SlackClientHyperConnector::new()?));

  let socket_mode_callbacks =
    SlackSocketModeListenerCallbacks::new().with_push_events(on_push_event);

  let listener_environment = Arc::new(
    SlackClientEventsListenerEnvironment::new(client.clone())
      .with_error_handler(test_error_handler),
  );

  let socket_mode_listener = SlackClientSocketModeListener::new(
    &SlackClientSocketModeConfig::new(),
    listener_environment.clone(),
    socket_mode_callbacks,
  );

  let app_token_value: SlackApiTokenValue = config_env_var("SLACK_TEST_APP_TOKEN")?.into();
  let app_token: SlackApiToken = SlackApiToken::new(app_token_value);

  socket_mode_listener.listen_for(&app_token).await?;

  socket_mode_listener.serve().await;

  Ok(())
}

pub fn config_env_var(name: &str) -> Result<String, String> {
  std::env::var(name).map_err(|e| format!("{}: {}", name, e))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  dotenvy::dotenv().ok();
  let subscriber = tracing_subscriber::fmt()
    .with_env_filter("slack_morphism=debug")
    .finish();
  tracing::subscriber::set_global_default(subscriber)?;

  test_client_with_socket_mode().await?;

  Ok(())
}