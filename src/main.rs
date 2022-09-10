mod smoke_test;
mod means_to_an_end;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tokio::join!(
        smoke_test::main(8080),
        means_to_an_end::main(8081)
    );
    Ok(())
}