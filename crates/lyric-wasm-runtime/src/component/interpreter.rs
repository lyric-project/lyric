use super::{Handler, Instance};
use crate::capability::rpc_task;
use crate::new_store;
use anyhow::Context;
use bytes::Bytes;
use std::future::Future;

pub mod wasmtime_handler_bindings {
    wasmtime::component::bindgen!({
        world: "interpreter-task",
        async: true,
        with: {
           "lyric:task/types@0.2.0": crate::capability::task::types,
        },
    });
}

pub mod wrpc_handler_bindings {
    wit_bindgen_wrpc::generate!({
        world: "interpreter-task",
        generate_all,
        with: {
            "lyric:task/types@0.2.0": crate::capability::rpc_task::types,
           "lyric:task/interpreter-task@0.2.0": generate,
        }
    });
}

impl<H, C> wrpc_handler_bindings::exports::lyric::task::interpreter_task::Handler<C>
    for Instance<H, C>
where
    H: Handler,
    C: Send,
{
    async fn run(
        &self,
        _: C,
        rpc_task::types::InterpreterRequest {
            resources,
            protocol,
            lang,
            code,
        }: rpc_task::types::InterpreterRequest,
    ) -> anyhow::Result<Result<rpc_task::types::InterpreterResponse, String>> {
        let mut store = new_store(
            &self.engine,
            self.handler.clone(),
            self.max_execution_time,
            None,
            resources.map(|r| r.into()),
        )?;
        let script = wasmtime_handler_bindings::lyric::task::types::InterpreterRequest {
            resources: None,
            protocol,
            lang,
            code,
        };

        let pre = wasmtime_handler_bindings::InterpreterTaskPre::new(self.pre.clone())
            .context("failed to pre-instantiate `lyric:task/interpreter-task`")?;
        let bindings = pre.instantiate_async(&mut store).await?;
        let res = bindings
            .lyric_task_interpreter_task()
            .call_run(&mut store, &script)
            .await
            .context("failed to call `lyric:task/interpreter-task`");
        res.map(|rs| {
            rs.map(|r| rpc_task::types::InterpreterResponse {
                protocol: r.protocol,
                // Vec<u8> to Bytes
                data: r.data.into(),
            })
        })
    }

    async fn run1(
        &self,
        _: C,
        rpc_task::types::InterpreterRequest {
            resources,
            protocol,
            lang,
            code,
        }: rpc_task::types::InterpreterRequest,
        call_name: String,
        input: Bytes,
    ) -> anyhow::Result<Result<rpc_task::types::InterpreterOutputResponse, String>> {
        let mut store = new_store(
            &self.engine,
            self.handler.clone(),
            self.max_execution_time,
            None,
            resources.map(|r| r.into()),
        )?;
        let script = wasmtime_handler_bindings::lyric::task::types::InterpreterRequest {
            resources: None,
            protocol,
            lang,
            code,
        };
        let input = input.to_vec();

        let pre = wasmtime_handler_bindings::InterpreterTaskPre::new(self.pre.clone())
            .context("failed to pre-instantiate `lyric:task/interpreter-task`")?;
        let bindings = pre.instantiate_async(&mut store).await?;
        let res = bindings
            .lyric_task_interpreter_task()
            .call_run1(&mut store, &script, &call_name, &input)
            .await
            .context("failed to call `lyric:task/interpreter-task`");
        res.map(|rs| {
            rs.map(|r| rpc_task::types::InterpreterOutputResponse {
                protocol: r.protocol,
                // Vec<u8> to Bytes
                data: r.data.into(),
                output: r.output.into(),
            })
        })
    }
}
