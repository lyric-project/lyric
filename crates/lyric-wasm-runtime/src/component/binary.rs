use super::{Handler, Instance};
use crate::new_store;
use anyhow::Context;
use std::future::Future;

pub mod wasmtime_handler_bindings {
    wasmtime::component::bindgen!({
        world: "binary-task",
        async: true,
        with: {
           "lyric:task/types@0.2.0": crate::capability::task::types,
        },
    });
}

pub mod wrpc_handler_bindings {
    wit_bindgen_wrpc::generate!({
        world: "binary-task",
        generate_all,
        with: {
            "lyric:task/types@0.2.0": generate,
           "lyric:task/binary-task@0.2.0": generate,
        }
    });
}

impl<H, C> wrpc_handler_bindings::exports::lyric::task::binary_task::Handler<C> for Instance<H, C>
where
    H: Handler,
    C: Send,
{
    async fn run(
        &self,
        _: C,
        wrpc_handler_bindings::lyric::task::types::BinaryRequest {
            protocol,
            data
        }: wrpc_handler_bindings::lyric::task::types::BinaryRequest,
    ) -> anyhow::Result<Result<wrpc_handler_bindings::lyric::task::types::BinaryResponse, String>>
    {
        let mut store = new_store(
            &self.engine,
            self.handler.clone(),
            self.max_execution_time,
            None,
        );
        let request = wasmtime_handler_bindings::lyric::task::types::BinaryRequest {
            protocol,
            // Bytes to Vec<u8>
            data: data.into(),
        };

        let pre = wasmtime_handler_bindings::BinaryTaskPre::new(self.pre.clone())
            .context("failed to pre-instantiate `lyric:task/interpreter-task`")?;
        let bindings = pre.instantiate_async(&mut store).await?;
        let res = bindings
            .lyric_task_binary_task()
            .call_run(&mut store, &request)
            .await
            .context("failed to call `lyric:task/interpreter-task`");
        res.map(|rs| {
            rs.map(
                |r| wrpc_handler_bindings::lyric::task::types::BinaryResponse {
                    protocol: r.protocol,
                    // Vec<u8> to Bytes
                    data: r.data.into(),
                },
            )
        })
    }
}
