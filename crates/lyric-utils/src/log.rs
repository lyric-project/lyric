use lazy_static::lazy_static;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::format::{FmtSpan, Writer};
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::fmt::time::{ChronoLocal, FormatTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

lazy_static! {
    static ref GLOBAL_TARGET: RwLock<String> = RwLock::new(String::new());
    static ref SUBSCRIBER_INITIALIZED: AtomicBool = AtomicBool::new(false);
}

pub fn read_target() -> String {
    GLOBAL_TARGET.read().unwrap().as_str().to_string()
}

pub fn set_target(target: String) -> String {
    let mut inner = GLOBAL_TARGET.write().unwrap();
    let old_value = inner.clone();
    *inner = target;
    old_value
}

pub fn init_tracing_subscriber<T: Into<String>, E: AsRef<str>>(target: T, default_level: E) {
    // If the subscriber has been initialized, return directly
    if SUBSCRIBER_INITIALIZED.load(Ordering::SeqCst) {
        return;
    }

    set_target(target.into());
    let env_filter = EnvFilter::try_from_env("LYRIC_CORE_LOG_LEVEL")
        .or(EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    let with_ansicolor = env::var("LYRIC_CORE_LOG_ANSICOLOR")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(true);

    let format = fmt::format()
        .with_level(true)
        .with_target(true)
        .with_thread_names(true)
        .with_timer(ChronoLocal::rfc_3339())
        .compact();

    let mut layers = Vec::new();

    // Add stdout layer
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .event_format(format.clone())
        .with_ansi(with_ansicolor)
        .with_span_events(FmtSpan::CLOSE);

    layers.push(stdout_layer.boxed());

    // Check if the log directory environment variable is configured
    if let Ok(log_dir) = env::var("LYRIC_CORE_LOG_DIR") {
        // Create a file appender that rotates every day
        let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir.clone(), "app.log");
        let (non_blocking, _guard) = NonBlocking::new(file_appender);

        // Add file layer in text format
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .event_format(format.clone())
            .with_ansi(false)
            .with_span_events(FmtSpan::CLOSE);

        layers.push(file_layer.boxed());

        // Create a file appender that rotates every day for trace logs
        let trace_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "trace.log");
        let (trace_non_blocking, _trace_guard) = NonBlocking::new(trace_appender);

        // Add trace layer in JSON format
        let trace_layer = fmt::layer()
            .json()
            .with_writer(trace_non_blocking)
            .with_current_span(true)
            .with_span_list(true)
            .with_timer(ChronoLocal::rfc_3339())
            .with_span_events(FmtSpan::CLOSE)
            .fmt_fields(fmt::format::JsonFields::new())
            .event_format(
                fmt::format()
                    .json()
                    .with_level(true)
                    .with_target(true)
                    .with_thread_names(true)
                    .with_timer(ChronoLocal::rfc_3339()),
            )
            .with_filter(filter_fn(|metadata| metadata.is_span()));

        layers.push(trace_layer.boxed());

        // Keep _guard and _trace_guard alive
        std::mem::forget(_guard);
        std::mem::forget(_trace_guard);
    }

    // Build and initialize the subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(layers)
        .init();

    // Set the subscriber initialization flag
    SUBSCRIBER_INITIALIZED.store(true, Ordering::SeqCst);
}

pub struct EventFormatter {
    formatter: fmt::format::Format,
}

impl EventFormatter {
    pub fn new() -> Self {
        Self {
            formatter: fmt::format::Format::default(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for EventFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let meta = event.metadata();

        SystemTime {}.format_time(&mut writer)?;
        writer.write_char(' ')?;

        let fmt_level = meta.level().as_str();
        write!(writer, "{:>5} ", fmt_level)?;

        write!(writer, "{:0>2?} ", std::thread::current().id())?;

        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                write!(writer, "{}", span.metadata().target())?;
                write!(writer, "@{}", span.metadata().name())?;
                write!(writer, "#{:x}", span.id().into_u64())?;

                let ext = span.extensions();
                if let Some(fields) = &ext.get::<FormattedFields<N>>() {
                    if !fields.is_empty() {
                        write!(writer, "{{{}}}", fields)?;
                    }
                }
                write!(writer, ": ")?;
            }
        };

        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}
