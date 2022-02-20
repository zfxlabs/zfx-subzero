#[cfg(test)]
#[cfg(feature = "integration_tests")]
mod integration_test {
    use crate::integration_test::cell_transfer_benchmark::run_cell_transfer_benchmark_test;
    use crate::integration_test::hail_integration_test::run_hail_integration_test;
    use crate::integration_test::sleet_integration_test::run_all_integration_tests;
    use crate::integration_test::stress_test::run_stress_test;
    use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
    use crate::Result;
    use std::thread::sleep;
    use std::time::Duration;

    #[actix_rt::test]
    async fn run_integration_test_suite() -> Result<()> {
        tracing_subscriber::fmt()
            .with_level(false)
            .with_target(false)
            .without_time()
            .compact()
            .with_max_level(tracing::Level::INFO)
            .init();

        run_all_integration_tests().await?;
        sleep(Duration::from_secs(5));
        run_stress_test().await?;
        sleep(Duration::from_secs(5));
        run_hail_integration_test().await?;
        sleep(Duration::from_secs(5));
        run_cell_transfer_benchmark_test().await?;

        Result::Ok(())
    }
}
