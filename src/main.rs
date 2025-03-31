use regex::Regex;
use std::collections::HashMap;
use std::env;

mod ai;
mod qdrant;

use crate::qdrant::all_documents;
use dotenv::dotenv;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Message;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let _ = tokio::task::spawn_blocking(|| init_qdrant().and_then(|_| print_docs())).await?;

    let bot = Bot::from_env();

    let user_states = Arc::new(Mutex::new(HashMap::<teloxide::types::ChatId, State>::new()));

    teloxide::repl(bot, move |message: Message, bot: Bot| {
        let user_states = user_states.clone();
        async move {
            if let Some(text) = message.text() {
                let chat_id = message.chat.id;
                let response_text = tokio::task::spawn_blocking({
                    let input = text.to_owned();
                    let user_states = user_states.clone();
                    move || {
                        let mut states = user_states.blocking_lock();
                        let state = states.entry(chat_id).or_insert(State::AwaitingPassword);
                        match State::process(&input, state) {
                            Ok((new_state, output)) => {
                                *state = new_state;
                                output
                            }
                            Err(err) => err.to_string(),
                        }
                    }
                })
                .await
                .unwrap_or_else(|err| err.to_string());
                bot.send_message(chat_id, response_text).await?;
            } else {
                bot.send_message(message.chat.id, "I did not understand what you said!")
                    .await?;
            }
            respond(())
        }
    })
    .await;
    Ok(())
}

enum State {
    AwaitingPassword,
    Pending,
    ConfirmForget { info: String },
    ConfirmCommand { message: String, command: String },
}

impl State {
    pub fn process(input: &str, state: &State) -> anyhow::Result<(Self, String)> {
        match state {
            State::AwaitingPassword => State::process_password(input),
            State::Pending => State::exec_pending(input),
            State::ConfirmForget { info } => State::exec_forget(input, info),
            State::ConfirmCommand { command, message } => {
                State::exec_confirm_command(input, command, message)
            }
        }
    }

    pub fn process_password(input: &str) -> anyhow::Result<(Self, String)> {
        let correct_password = env::var("BOT_PASSWORD")?;
        if input.trim() == correct_password {
            Ok((
                State::Pending,
                "Password accepted. You may continue using the bot.".to_string(),
            ))
        } else {
            Ok((
                State::AwaitingPassword,
                "Incorrect password. Please try again.".to_string(),
            ))
        }
    }

    pub fn exec_pending(message: &str) -> anyhow::Result<(Self, String)> {
        let user = format!(
            "<user_message>{}</user_message> Inside user_message there is: \n \
        1. a question (interrogative sentence) \n \
        2. affirmative information, data, facts or details \n \
        3. a sentence requesting to delete information from memory \n \
        4. a terminal command \n \
        5. other \n \
        Respond with a number. ",
            message
        );
        let response = ai::llm("Give a short answer without explanations or details", &user)?;
        // If there is a parsing error, return 5.
        let number = State::extract_number(&response).parse::<i32>().unwrap_or(5);

        match number {
            1 => State::exec_answer(message),
            2 => State::exec_remember(message),
            3 => State::new_forget(message),
            4 => State::new_command(message),
            _ => State::exec_chat(message),
        }
    }

    pub fn exec_answer(message: &str) -> anyhow::Result<(Self, String)> {
        let user = format!(
            "<user_request>{}</user_request> Extract the keywords from user_request \
         Respond in the format <keywords>KEYWORDS</keywords> ",
            message
        );
        let response = ai::llm("Give a short answer without explanations or details", &user)?;
        let keywords = State::extract_tag(&response, "keywords");
        let docs = qdrant::search_smart(&keywords)?;
        println!("Question: {}", message);
        for doc in &docs {
            println!("{}: {}", doc.distance, doc.text);
        }
        let docs_text = docs
            .iter()
            .map(|doc| doc.text.clone())
            .collect::<Vec<String>>()
            .join("\n\n");
        let user = format!("{}\n\n {}", docs_text, message);
        let response = ai::llm(
            "You are a friendly and helpful assistant. Start answering without a greeting.",
            &user,
        )?;
        Ok((State::Pending, response))
    }

    fn extract_tag(input: &str, tag: &str) -> String {
        let re = Regex::new(&format!(r"(?i)<{}>(.*?)</{}>", tag, tag)).unwrap();
        if let Some(caps) = re.captures(input) {
            caps.get(1)
                .map_or(String::new(), |m| m.as_str().to_string())
        } else {
            String::new()
        }
    }

    fn extract_number(input: &str) -> String {
        let re = Regex::new(r"\d+").unwrap();
        match re.find(input) {
            Some(m) => m.as_str().to_string(),
            None => String::new(),
        }
    }

    pub fn exec_chat(message: &str) -> anyhow::Result<(Self, String)> {
        let response = ai::llm(
            "You are a friendly and helpful assistant. Start answering without a greeting. \
         Preferably answer in one or no more than three sentences.",
            message,
        )?;
        Ok((State::Pending, response))
    }

    pub fn exec_remember(message: &str) -> anyhow::Result<(Self, String)> {
        let mut last_document_id = qdrant::last_document_id()?;
        last_document_id += 1;
        qdrant::add_document(last_document_id, message)?;
        Ok((State::Pending, "Information saved.".to_string()))
    }

    pub fn new_forget(message: &str) -> anyhow::Result<(Self, String)> {
        let user = format!(
            "<user_request>{}</user_request> Extract the keywords from user_request \
         Respond in the format <keywords>KEYWORDS</keywords> ",
            message
        );
        let response = ai::llm("Give a short answer without explanations or details", &user)?;
        let keywords = State::extract_tag(&response, "keywords");
        let doc = qdrant::search_one(&keywords)?;
        let text = doc.text.clone();
        Ok((
            State::ConfirmForget { info: text.clone() },
            format!("'{}' Forget this information?", text),
        ))
    }

    pub fn exec_forget(message: &str, info: &str) -> anyhow::Result<(Self, String)> {
        if State::is_condition(message, "consent")? {
            let doc = qdrant::search_one(info)?;
            qdrant::delete_document(doc.id)?;
            Ok((State::Pending, "Information forgotten.".to_string()))
        } else {
            Ok((State::Pending, "Information not forgotten.".to_string()))
        }
    }

    pub fn new_command(message: &str) -> anyhow::Result<(Self, String)> {
        let user = format!(
            "<user_request>{}</user_request> Based on the user_request description, I will form a Linux command for the terminal. \
             Respond in the format <command>COMMAND</command>",
            message
        );
        let response = ai::llm("Give a short answer without explanations or details", &user)?;
        let command = State::extract_tag(&response, "command");
        Ok((
            State::ConfirmCommand {
                command: command.clone(),
                message: message.to_string(),
            },
            format!("Run command \"{}\"?", command),
        ))
    }

    pub fn exec_confirm_command(
        message: &str,
        command: &str,
        priv_message: &str,
    ) -> anyhow::Result<(Self, String)> {
        if State::is_condition(message, "yes")? {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output();
            match output {
                Ok(result) => {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    let mut ret = format!("Command execution result\n```\n{}\n```", stdout);
                    if !stderr.is_empty() {
                        ret = format!("Errors during command execution\n```\n{}\n```", stderr);
                    }
                    Ok((State::Pending, ret))
                }
                Err(e) => Ok((State::Pending, format!("Error executing command: {}", e))),
            }
        } else if message.len() > 7 {
            let message = format!("{}\n{}", priv_message, message);
            State::new_command(&message)
        } else {
            println!("Command not executed.");
            Ok((State::Pending, "Command not executed.".to_string()))
        }
    }

    pub fn is_condition(message: &str, condition: &str) -> anyhow::Result<bool> {
        let user = format!(
            "<user_request>{}</user_request> Does user_request contain {}? \
         Respond in the format <response>yes</response> or <response>no</response>",
            message, condition
        );
        let response = ai::llm("Give a short answer without explanations or details", &user)?;
        Ok(response.to_lowercase().contains("yes"))
    }
}

fn init_qdrant() -> anyhow::Result<()> {
    if !qdrant::exists_collection()? {
        qdrant::create_collection()?;
    }
    Ok(())
}

fn print_docs() -> anyhow::Result<()> {
    // Print bot's memory contents
    let docs = all_documents()?;
    for doc in docs {
        println!("{}", doc.text);
    }
    Ok(())
}
