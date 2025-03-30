use std::env;
use regex::Regex;

mod ai;
mod dr;

use crate::dr::all_documents;
use dotenv::dotenv;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Message;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    dotenv().ok();

    tokio::task::spawn_blocking(|| {

        if !dr::exists_collection().unwrap() {
            dr::create_collection().unwrap();
        }

        let docs = all_documents().unwrap();
        for doc in docs {
            println!("{}", doc.text);
        }
    })
    .await
    .unwrap();

    let bot = Bot::from_env();

    let state = Arc::new(Mutex::new(State::AwaitingPassword));

    teloxide::repl(bot, move |message: Message, bot: Bot| {
        let state = state.clone();
        async move {
            if let Some(text) = message.text() {
                let response_text = tokio::task::spawn_blocking({
                    let input = text.to_owned();
                    let state = state.clone();
                    move || {
                        let mut state_guard = state.blocking_lock();
                        let (new_state, output) = State::process(&input, &state_guard);
                        *state_guard = new_state;
                        output
                    }
                })
                .await
                .unwrap();

                bot.send_message(message.chat.id, response_text).await?;
            } else {
                bot.send_message(message.chat.id, "Не понял, что ты сказал!")
                    .await?;
            }
            respond(())
        }
    })
    .await;
}

enum State {
    Pending,
    ConfirmForget { info: String },
    ConfirmCommand { message: String, command: String },
    AwaitingPassword,
}

impl State {
    pub fn process(input: &str, state: &State) -> (Self, String) {
        match state {
            State::AwaitingPassword => State::process_password(input),
            State::Pending => State::exec_pending(input),
            State::ConfirmForget { info } => State::exec_forget(input, info),
            State::ConfirmCommand { command, message } => {
                State::exec_confirm_command(input, command, message)
            }
        }
    }

    pub fn process_password(input: &str) -> (Self, String) {
        let correct_password = env::var("BOT_PASSWORD").expect("BOT_PASSWORD не установлен!");
        if input.trim() == correct_password {
            (
                State::Pending,
                "Пароль принят. Вы можете продолжать работу с ботом.".to_string(),
            )
        } else {
            (
                State::AwaitingPassword,
                "Неверный пароль. Попробуйте снова.".to_string(),
            )
        }
    }
    pub fn exec_pending(message: &str) -> (Self, String) {
        let user = format!(
            "<user_request>{}</user_request> В user_request содержится: \n \
        1. вопросительное предложение (вопрос) \n \
        2. утвердительная информация, данные, факты или сведения \n \
        3. предложение с просьбой удалить информацию из памяти \n \
        4. команда для терминала \n \
        5. другое \n \
        Ответь цифрой. ",
            message
        );
        // println!("{}", user);
        let response = ai::llm("Дай короткий ответ без объяснений и деталей", &user).unwrap();
        // println!("{}", response);
        // если ошибка парсинга то возвращаем 5
        let number = State::extract_number(&response).parse::<i32>().unwrap_or(5);

        match number {
            1 => State::exec_answer(message),
            2 => State::exec_remember(message),
            3 => State::new_forget(message),
            4 => State::new_command(message),
            _ => State::exec_chat(message),
        }
    }

    pub fn exec_answer(message: &str) -> (Self, String) {
        let user = format!(
            "<user_request>{}</user_request> Извлеки из user_request ключевые слова \
         Ответь в формате <keywords>КЛЮЧЕВЫЕ СЛОВА</keywords> ",
            message
        );
        let response = ai::llm("Дай короткий ответ без объяснений и деталей", &user).unwrap();
        let keywords = State::extract_tag(&response, "keywords");
        let docs = dr::search_smart(&keywords).unwrap();
        println!("Вопрос: {}", message);
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
            "Ты дружелюбный и полезный помощник. Начни отвечать без приветствия.",
            &user,
        )
        .unwrap();
        // println!("{}", response);
        (State::Pending, response)
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

    pub fn exec_chat(message: &str) -> (Self, String) {
        let response = ai::llm("Ты дружелюбный и полезный помощник. Начни отвечать без приветствия. Отвечай желательно одним или не больше трех предложений.", message).unwrap();
        // println!("{}", response);
        (State::Pending, response)
    }

    pub fn exec_remember(message: &str) -> (Self, String) {
        let mut last_document_id = dr::last_document_id().unwrap();
        last_document_id += 1;
        dr::add_document(last_document_id, message).unwrap();
        // println!("Информация сохранена.");
        (State::Pending, "Информация сохранена.".to_string())
    }
    pub fn new_forget(message: &str) -> (Self, String) {
        let user = format!(
            "<user_request>{}</user_request> Извлеки из user_request ключевые слова \
         Ответь в формате <keywords>КЛЮЧЕВЫЕ СЛОВА</keywords> ",
            message
        );
        let response = ai::llm("Дай короткий ответ без объяснений и деталей", &user).unwrap();
        let keywords = State::extract_tag(&response, "keywords");
        let doc = dr::search_one(&keywords).unwrap();
        let text = doc.text.clone();
        // println!("'{}' Забыть информацию?", text);
        (
            State::ConfirmForget { info: text.clone() },
            format!("'{}' Забыть информацию?", text),
        )
    }

    pub fn exec_forget(message: &str, info: &str) -> (Self, String) {
        if State::is_condition(message, "согласие") {
            let doc = dr::search_one(info).unwrap();
            dr::delete_document(doc.id).unwrap();
            // println!("Информация забыта.");
            (State::Pending, "Информация забыта.".to_string())
        } else {
            // println!("Информация не забыта.");
            (State::Pending, "Информация не забыта.".to_string())
        }
        // State::Pending
    }

    // Новая логика для обработки команд терминала.
    pub fn new_command(message: &str) -> (Self, String) {
        let user = format!(
            "<user_request>{}</user_request> На основе описание user_request сформирую linux команду для терминала. \
             Ответь в формате <command>КОМАНДА</command>", message
        );
        let response = ai::llm("Дай короткий ответ без объяснений и деталей", &user).unwrap();
        let command = State::extract_tag(&response, "command");
        // println!("Запустить команду \"{}\" ?", command);
        (
            State::ConfirmCommand {
                command: command.clone(),
                message: message.to_string(),
            },
            format!("Запустить команду \"{}\" ?", command),
        )
    }

    pub fn exec_confirm_command(
        message: &str,
        command: &str,
        priv_message: &str,
    ) -> (Self, String) {
        if State::is_condition(message, "yes") {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output();
            match output {
                Ok(result) => {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    let mut ret = format!("Результат работы команды\n```\n{}\n```", stdout);
                    if !stderr.is_empty() {
                        ret = format!("Ошибки при выполнении команды\n```\n{}\n```", stderr);
                    }
                    (State::Pending, ret)
                }
                Err(e) => {
                    // println!("Ошибка при выполнении команды: {}", e);
                    (
                        State::Pending,
                        format!("Ошибка при выполнении команды: {}", e),
                    )
                }
            }
        } else if message.len() > 7 {
            let message = format!("{}\n{}", priv_message, message);
            State::new_command(&message)
        } else {
            println!("Команда не выполнена.");
            (State::Pending, "Команда не выполнена.".to_string())
        }
    }

    pub fn is_condition(message: &str, condition: &str) -> bool {
        let user = format!(
            "<user_request>{}</user_request> В user_request содержится {}? \
         Ответь в формате <response>yes</response> или <response>no</response>",
            message, condition
        );
        // println!("{}", user);
        let response = ai::llm("Дай короткий ответ без объяснений и деталей", &user).unwrap();
        // println!("{}", response);
        response.to_lowercase().contains("yes")
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn test_extract_keywords() {
        let input = "asdsad<keywords>КЛЮЧЕВЫЕ СЛОВА</keywords>asdas";
        let output = State::extract_tag(input, "keywords");
        assert_eq!(output, "КЛЮЧЕВЫЕ СЛОВА");
    }
}
