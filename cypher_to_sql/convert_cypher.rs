use sqlparser::ast::Statement;
use sqlparser::cypher_to_sql;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cypher_query = &args[1];
    
    match convert_cypher_to_sql(cypher_query) {
        Ok(sql) => {
            println!("{}", sql);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn convert_cypher_to_sql(cypher: &str) -> Result<String, String> {
    let dialect = GenericDialect {};
    
    // Parse the Cypher query
    let ast = Parser::parse_sql(&dialect, cypher)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Extract the CypherQuery statement
    match ast.first() {
        Some(Statement::CypherQuery {
            pattern,
            where_clause,
            return_items,
        }) => {
            // Convert to SQL
            let sql_stmt = cypher_to_sql::cypher_to_sql(pattern, where_clause, return_items)
                .map_err(|e| format!("Conversion error: {}", e))?;
            
            Ok(sql_stmt.to_string())
        }
        Some(Statement::CypherCreate { pattern }) => {
            // Convert CREATE to INSERT
            let sql_stmt = cypher_to_sql::cypher_create_to_sql(pattern)
                .map_err(|e| format!("Conversion error: {}", e))?;
            
            Ok(sql_stmt.to_string())
        }
        Some(_) => Err("Not a Cypher query".to_string()),
        None => Err("No statement parsed".to_string()),
    }
}