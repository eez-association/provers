//! The default evm2 follower and stateful signing service share one database.

use eez_prover_stateful::Config;

fn main() -> eyre::Result<()> {
    eez_follower::run_with(|args| {
        eyre::ensure!(
            args.sequencer_address.is_none(),
            "stateful proving requires L1-derived heads; remove EEZ_SEQUENCER_ADDRESS",
        );
        let config = Config::from_env()?;
        Ok(move |context: eez_follower::FollowerContext| {
            context
                .task_executor
                .spawn_critical_with_graceful_shutdown_signal(
                    "eez-stateful-proof-signer",
                    move |shutdown| async move {
                        let signal = shutdown.clone().ignore_guard();
                        let result = eez_prover_stateful::serve(
                            config,
                            context.provider,
                            context.chain_spec,
                            context.committer,
                            signal,
                        )
                        .await;
                        // Keep Reth's shutdown guard until tonic has drained.
                        drop(shutdown);
                        result.unwrap_or_else(|error| {
                            panic!("stateful proof signer exited: {error:#}")
                        });
                    },
                );
            Ok(())
        })
    })
}
