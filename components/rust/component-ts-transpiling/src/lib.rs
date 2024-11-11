use deno_ast::{
    EmitOptions, MediaType, ModuleSpecifier, ParseParams, SourceMapOption, TranspileModuleOptions,
};
use exports::lyric::transpiling::typescript_transpile::{TranspileRequest, TranspileResponse};
use std::error::Error;

fn transpile_ts(code: &str) -> Result<String, Box<dyn Error>> {
    let params = ParseParams {
        specifier: ModuleSpecifier::parse("file:///input.ts").unwrap(),
        text: code.into(),
        media_type: MediaType::TypeScript,
        // Capture tokens, for syntax highlighting, set to false to improve performance
        capture_tokens: false,
        // Whether analyze the scope of the module
        scope_analysis: false,
        // Custom syntax configuration
        maybe_syntax: None,
    };

    let parsed = deno_ast::parse_module(params)?;

    // Set default options
    let transpile_options = Default::default();
    let module_options = TranspileModuleOptions::default();
    let emit_options = EmitOptions {
        source_map: SourceMapOption::None,
        ..Default::default()
    };

    let result = parsed.transpile(&transpile_options, &module_options, &emit_options)?;

    Ok(result.into_source().text)
}

struct MyTask;

impl exports::lyric::transpiling::typescript_transpile::Guest for MyTask {
    fn transpile(req: TranspileRequest) -> Result<TranspileResponse, String> {
        match transpile_ts(&req.text) {
            Ok(text) => Ok(TranspileResponse { text }),
            Err(e) => Err(e.to_string()),
        }
    }
}

wit_bindgen::generate!({ generate_all });

export!(MyTask);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_types_and_interface() {
        let ts_code = r#"
            interface Person {
                name: string;
                age: number;
            }

            function greet(person: Person): string {
                return `Hello, ${person.name}! You are ${person.age} years old.`;
            }
            console.log(greet({ name: "Alice", age: 30 }));
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify type information removal
        assert!(!result.contains("interface"));
        assert!(!result.contains(": Person"));
        assert!(!result.contains(": string"));

        // Verify functionality preservation
        assert!(result.contains("function greet"));
        assert!(result.contains("return `Hello, ${person.name}! You are ${person.age} years old.`"));
    }

    #[test]
    fn test_class_and_inheritance() {
        let ts_code = r#"
            class Animal {
                protected name: string;

                constructor(name: string) {
                    this.name = name;
                }

                makeSound(): string {
                    return "Some sound";
                }
            }

            class Dog extends Animal {
                private age: number;

                constructor(name: string, age: number) {
                    super(name);
                    this.age = age;
                }

                makeSound(): string {
                    return "Woof!";
                }
            }
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify type information removal
        assert!(!result.contains(": string"));
        assert!(!result.contains(": number"));
        assert!(!result.contains("private"));
        assert!(!result.contains("protected"));

        // Verify class structure preservation
        assert!(result.contains("class Animal"));
        assert!(result.contains("class Dog extends Animal"));
        assert!(result.contains("super(name)"));
        assert!(result.contains("makeSound()"));
    }

    #[test]
    fn test_generics() {
        let ts_code = r#"
            function identity<T>(arg: T): T {
                return arg;
            }

            const result: string = identity("hello");
            const numResult: number = identity(42);
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify generic type information removal
        assert!(!result.contains("<T>"));
        assert!(!result.contains(": T"));
        assert!(!result.contains(": string"));
        assert!(!result.contains(": number"));

        // Verify functionality preservation
        assert!(result.contains("function identity"));
        assert!(result.contains("return arg"));
    }

    #[test]
    fn test_async_await() {
        let ts_code = r#"
            async function fetchData(): Promise<string> {
                const response = await fetch('https://api.example.com/data');
                const data: string = await response.text();
                return data;
            }

            async function processData(): Promise<void> {
                try {
                    const result = await fetchData();
                    console.log(result);
                } catch (error) {
                    console.error(error);
                }
            }
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify type information removal
        assert!(!result.contains(": Promise<string>"));
        assert!(!result.contains(": Promise<void>"));
        assert!(!result.contains(": string"));

        // Verify async/await preservation
        assert!(result.contains("async function"));
        assert!(result.contains("await fetch"));
        assert!(result.contains("try {"));
        assert!(result.contains("catch"));
    }

    #[test]
    fn test_decorators() {
        let ts_code = r#"
            function log(target: any, propertyKey: string) {
                console.log(`Accessing property: ${propertyKey}`);
            }

            class Example {
                @log
                get name(): string {
                    return "example";
                }
            }
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify decorator and type information removal
        assert!(!result.contains(": string"));
        assert!(!result.contains(": any"));

        // Verify basic functionality preservation
        assert!(result.contains("class Example"));
        assert!(result.contains("get name"));
    }

    #[test]
    fn test_empty_input() {
        let result = transpile_ts("").unwrap();
        assert_eq!(result.trim(), "");
    }

    #[test]
    fn test_complex_types() {
        let ts_code = r#"
            type ComplexType = {
                id: number;
                data: Array<string>;
                metadata: {
                    created: Date;
                    modified?: Date;
                };
                process: (input: string) => number;
            };

            const processor: ComplexType = {
                id: 1,
                data: ["test"],
                metadata: {
                    created: new Date()
                },
                process: (input) => input.length
            };
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify type information removal
        assert!(!result.contains("type ComplexType"));
        assert!(!result.contains(": number"));
        assert!(!result.contains(": Array<string>"));
        assert!(!result.contains(": Date"));

        // Verify object structure preservation
        assert!(result.contains("const processor"));
        assert!(result.contains("id: 1"));
        assert!(result.contains("data: ["));
        assert!(result.contains("new Date()"));
    }

    #[test]
    fn test_syntax_error() {
        let ts_code = "const x: number = 'invalid"; // Missing closing quote
        let result = transpile_ts(ts_code);
        assert!(result.is_err());
    }

    #[test]
    fn test_enum_transpilation() {
        let ts_code = r#"
            enum Direction {
                Up = "UP",
                Down = "DOWN",
                Left = "LEFT",
                Right = "RIGHT"
            }

            const currentDirection: Direction = Direction.Up;
        "#;

        let result = transpile_ts(ts_code).unwrap();

        // Verify enum is properly transformed
        assert!(result.contains("Direction"));
        assert!(result.contains(r#""UP""#));
        assert!(!result.contains(": Direction"));
    }
}
