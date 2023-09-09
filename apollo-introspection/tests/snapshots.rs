use apollo_compiler::ApolloCompiler;
use apollo_compiler::ReprDatabase;
use apollo_introspection::SchemaIntrospectionQuery;
use apollo_introspection::VariableValues;
use expect_test::expect;
use expect_test::expect_file;
use serde_json_bytes::json;
use serde_json_bytes::Value as JsonValue;

#[test]
fn test() {
    let schema = r#"
        type Query implements I { 
            id: ID!
            int: Int! @deprecated(reason: "…")
            url: Url
        }

        interface I {
            id: ID!
        }

        scalar Url @specifiedBy(url: "https://url.spec.whatwg.org/")
    "#;
    let mut compiler = ApolloCompiler::new();
    compiler.add_type_system(schema, "schema.graphql");
    let id = compiler.add_executable("", "query.graphql");

    let mut introspect = |query, variables| {
        compiler.update_executable(id, query);
        let errors = compiler.validate();
        if !errors.is_empty() {
            for error in errors {
                println!("{}", error)
            }
            panic!("Validation failed")
        }
        let schema = compiler.db.schema();
        let mut document = compiler.db.executable_document(id).unwrap();
        let operation = document.get_operation(None).unwrap();
        let variables = JsonValue::as_object(&variables).unwrap();
        let variables = VariableValues::coerce(&schema, &operation, &variables).unwrap();
        let introspection = SchemaIntrospectionQuery::split_from(&mut document, None).unwrap();
        let response = introspection.execute_sync(&schema, &variables).unwrap();
        serde_json::to_string_pretty(&response).unwrap()
    };

    let response = introspect(include_str!("introspect_full_schema.graphql"), json!({}));
    expect_file!("response_full.json").assert_eq(&response);

    let query = r#"
        query WithVarible($verbose: Boolean!) {
            I: __type(name: "I") {
                possibleTypes {
                    name
                    fields @skip(if: $verbose) {
                        name
                    }
                    verboseFields: fields(includeDeprecated: true) @include(if: $verbose) {
                        name
                        deprecationReason
                    }
                }
            }
            Url: __type(name: "Url") @include(if: $verbose) {
                specifiedByURL
            }
        }
    "#;
    let expected = expect!([r#"
        {
          "data": {
            "I": {
              "possibleTypes": [
                {
                  "name": "Query",
                  "fields": [
                    {
                      "name": "id"
                    },
                    {
                      "name": "url"
                    }
                  ]
                }
              ]
            }
          }
        }"#]);
    let response = introspect(query, json!({"verbose": false}));
    expected.assert_eq(&response);

    let response = introspect(query, json!({"verbose": true}));
    let expected = expect!([r#"
        {
          "data": {
            "I": {
              "possibleTypes": [
                {
                  "name": "Query",
                  "verboseFields": [
                    {
                      "name": "id",
                      "deprecationReason": null
                    },
                    {
                      "name": "int",
                      "deprecationReason": "…"
                    },
                    {
                      "name": "url",
                      "deprecationReason": null
                    }
                  ]
                }
              ]
            },
            "Url": {
              "specifiedByURL": "https://url.spec.whatwg.org/"
            }
          }
        }"#]);
    expected.assert_eq(&response);
}
