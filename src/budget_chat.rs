use std::collections::HashSet;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{self, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub async fn main(port: u32) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("budget_chat is listening on port {}", port);

    let (tx, _) = broadcast::channel::<Message>(1024);
    let people_db: PeopleDB = Arc::new(Mutex::new(HashSet::new()));

    loop {
        let (socket, _) = listener.accept().await?;
        let mut people_db = people_db.clone();
        let mut tx = tx.clone();
        tokio::spawn(async move {
            if let Result::Err(e) = handle_session(&mut people_db, &mut tx, socket).await {
                println!("Diconnected: {:?}", e)
            }
        });
    }
}

type PeopleDB = Arc<Mutex<HashSet<String>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Message {
    from: String,
    content: String,
}

async fn handle_session(
    people_db: &mut PeopleDB,
    chat_publisher: &mut Sender<Message>,
    conn: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut reader, mut writer) = tokio::io::split(conn);

    // join
    chat_publisher.subscribe();
    socket_write_line(&mut writer, "Name?").await?;
    let name = socket_read_line(&mut reader).await?;
    let (can_join, msg) = join(people_db, chat_publisher, &name).await;
    socket_write_line(&mut writer, &msg).await?;
    if !can_join {
        return Result::Err("validation failed and rejected from joining".into());
    }

    // start write loop
    let cloned_name = name.clone();
    let mut receiver = chat_publisher.subscribe();
    let write_loop: JoinHandle<Result<(), String>> = tokio::spawn(async move {
        loop {
            let Message { from, content } = receiver.recv().await.map_err(|e| e.to_string())?;
            if from != cloned_name {
                socket_write_line(&mut writer, &content)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    });

    // start read loop
    let cloned_name = name.clone();
    let mut cloned_chat_publisher = chat_publisher.clone();
    let read_loop: JoinHandle<Result<(), String>> = tokio::spawn(async move {
        loop {
            let line = socket_read_line(&mut reader)
                .await
                .map_err(|e| e.to_string())?;
            chat(&mut cloned_chat_publisher, &cloned_name, &line).await;
        }
    });

    // wait until any one of the loop finished
    let result = tokio::select! {
        result = read_loop => result?,
        result = write_loop => result?
    };

    leave(people_db, chat_publisher, &name).await;

    result.map_err(|e| e.into())
}

async fn socket_write_line(
    socket: &mut WriteHalf<TcpStream>,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    socket
        .write_all(format!("{}\n", content).as_bytes())
        .await
        .map_err(Into::into)
}

async fn socket_read_line(
    socket: &mut ReadHalf<TcpStream>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut segments = vec![];
    let mut buf = [0u8; 32];
    loop {
        let n = socket.read(&mut buf).await?;
        if n == 0 {
            return Result::Err("Read 0 bytes".into());
        } else if (buf[n - 1] as char) == '\n' {
            segments.extend_from_slice(&buf[0..n - 1]);
            break;
        } else {
            segments.extend_from_slice(&buf[0..n]);
        }
    }
    std::str::from_utf8(&segments)
        .map(|s| s.trim_end().to_string())
        .map_err(Into::into)
}

async fn join(
    people_db: &mut PeopleDB,
    chat_publisher: &mut Sender<Message>,
    name: &str,
) -> (bool, String) {
    if name.len() < 1 || !name.chars().all(char::is_alphanumeric) {
        return (false, format!("Name {} is invalid", name));
    }

    let msg = {
        let mut db = people_db.lock().await;
        if db.contains(name) {
            return (false, "Name is taken".into());
        }

        let msg = format!(
            "* The room contains: {}",
            db.iter().cloned().collect::<Vec<String>>().join(", ")
        );
        db.insert(name.to_string());
        msg
    };

    let content = format!("* {} has entered the room", name);
    let _ = chat_publisher.send(Message {
        from: "".to_string(),
        content,
    });

    (true, msg)
}

async fn chat(chat_publisher: &mut Sender<Message>, name: &str, content: &str) {
    if content.is_empty() {
        return;
    }

    let content = format!("[{}] {}", name, content);
    let _ = chat_publisher.send(Message {
        from: name.to_string(),
        content,
    });
}

async fn leave(people_db: &mut PeopleDB, chat_publisher: &mut Sender<Message>, name: &str) {
    let is_removed = {
        let mut db = people_db.lock().await;
        db.remove(name)
    };

    if is_removed {
        let content = format!("* {} has left the room", name);
        let _ = chat_publisher.send(Message {
            from: "".to_string(),
            content,
        });
    }
}
