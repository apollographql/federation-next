use apollo_harness::core::Cli;

use clap::Parser;
use router_bridge::planner::Planner;
use router_bridge::planner::QueryPlannerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let schema = tokio::fs::read_to_string(cli.schema).await?;
    let planner = Planner::<serde_json::Value>::new(schema, QueryPlannerConfig::default())
        .await
        .unwrap();

    if let Some(query_file) = cli.query {
        let query = tokio::fs::read_to_string(query_file).await?;
        let _payload = planner
            .plan(query, None)
            .await
            .unwrap()
            .into_result()
            .unwrap();
    }
    Ok(())
}
