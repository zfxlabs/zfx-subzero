#[cfg(test)]
#[cfg(feature = "integration_tests")]
mod integration_test {
    use crate::integration_test::integration_test::run_all_integration_tests;
    use crate::integration_test::sleet_benchmark::run_benchmark_test;
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

        Result::Ok(())
    }
}
