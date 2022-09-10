use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

//// Controller

pub async fn main(port: u32) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("means_to_an_end is listening on port {}", port);

    loop {
        let (socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            handle_conn(socket).await
        });
    }
}

type RawInputMessage = [u8; 9];

enum Message {
    Insert(InsertInput),
    Query(QueryInput)
}

async fn handle_conn(mut socket: TcpStream) {
    let mut in_buf: RawInputMessage = [0; 9];
    let mut db: Db = Default::default();

    loop {
        if let Err(e) = socket.read_exact(&mut in_buf).await {
            eprintln!("failed to read from socket; err = {:?}", e);
            return;
        }

        match parse_message(&in_buf) {
            Err(_) => {
                eprintln!("failed to parse message {:?}", in_buf);
                return;
            } ,
            Ok(Message::Insert(input)) => {
                insert(&mut db, input);
            },
            Ok(Message::Query(input)) => {
                let output = query(&db, input);
                if let Err(e) = socket.write(&output.mean.to_be_bytes()).await {
                    eprintln!("failed to write to socket; err = {:?}", e);
                    return;
                }
            }
        }
    }
}

fn parse_message(buf: &RawInputMessage) -> Result<Message, ()> {
    fn from_byte_slice(src: &[u8]) -> i32 {
        let mut dst = [0u8; 4];
        dst.clone_from_slice(src);
        i32::from_be_bytes(dst)
    }
    match buf[0] as char {
        'I' => Ok(Message::Insert(InsertInput {
            timestamp: from_byte_slice(&buf[1..5]),
            price: from_byte_slice(&buf[5..9]),
        })),
        'Q' => Ok(Message::Query(QueryInput {
            min_time: from_byte_slice(&buf[1..5]),
            max_time: from_byte_slice(&buf[5..9]),
        })),
        _ => Err(())
    }
}

//// Domain

struct InsertInput {
    timestamp: i32,
    price: i32,
}

struct QueryInput {
    min_time: i32,
    max_time: i32,
}

struct QueryOutput {
    mean: i32,
}

//// Database

type Db = Vec<InsertInput>;

fn insert(db: &mut Db, input: InsertInput) {
    db.push(input);
}

fn query(db: &Db, input: QueryInput) -> QueryOutput {
    let (cur_sum, cur_count): (i64, i64) = db
        .iter()
        .filter(|x| input.min_time <= x.timestamp && x.timestamp <= input.max_time)
        .fold((0, 0), |(cur_sum, cur_count), acc| {
            (cur_sum + (acc.price as i64), cur_count + 1)
        });
    QueryOutput {
        mean: if cur_count == 0 { 0 } else { cur_sum / cur_count } as i32
    }
}
