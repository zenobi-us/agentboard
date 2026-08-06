#[tokio::main]
async fn main() {
    match agentboard::cli::run().await {
        Ok(()) => {}
        Err(error)
            if error
                .downcast_ref::<agentboard::runtime::InvocationCancelled>()
                .is_some() =>
        {
            std::process::exit(130);
        }
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
    }
}
