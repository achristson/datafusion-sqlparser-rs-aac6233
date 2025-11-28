use crate::ast::*;
use crate::ast::helpers::attached_token::AttachedToken;

pub fn cypher_to_sql(
    pattern: &str,
    where_clause: &Option<Expr>,
    return_items: &[SelectItem],
) -> Result<Statement, String> {
    let table_name = extract_first_label(pattern)?;
    let table_alias = extract_first_variable(pattern);

    let from_table = TableWithJoins {
        relation: TableFactor::Table {
            name: ObjectName(vec![ObjectNamePart::Identifier(Ident::new(table_name))]),
            alias: table_alias.map(|var| TableAlias{
                name: Ident::new(var),
                columns: vec![],
            }),
            args: None,
            with_hints: vec![],
            version: None,
            with_ordinality: false,
            partitions: vec![],
            json_path: None,
            index_hints: vec![],
            sample: None,
        },
        joins: vec![],
    };

    let sql_projection = convert_return_items(return_items);

    let select = create_select(
        sql_projection,
        vec![from_table],
        where_clause.clone(),
    );

    Ok(Statement::Query(Box::new(Query {
        with: None,
        body: Box::new(SetExpr::Select(Box::new(select))),
        order_by: None,
        limit_clause: None,
        fetch: None,
        locks: vec![],
        for_clause: None,
        settings: None,
        format_clause: None,
        pipe_operators: vec![],
    })))
}

pub fn cypher_create_to_sql(pattern: &str) -> Result<Statement, String> {
    // Extract table name (label)
    let table_name = extract_first_label(pattern)?;
    
    // Extract properties from the pattern
    let (columns, values) = extract_properties(pattern)?;
    
    if columns.is_empty() {
        return Err("No properties found in CREATE statement".to_string());
    }
    
    // Build INSERT statement
    Ok(Statement::Insert(Insert {
        or: None,
        ignore: false,
        into: true,
        table: TableObject::TableName(ObjectName(vec![ObjectNamePart::Identifier(Ident::new(table_name))])),
        table_alias: None,
        columns,
        overwrite: false,
        source: Some(Box::new(Query {
            with: None,
            body: Box::new(SetExpr::Values(Values {
                explicit_row: false,
                rows: vec![values],
            })),
            order_by: None,
            limit_clause: None,
            fetch: None,
            locks: vec![],
            for_clause: None,
            settings: None,
            format_clause: None,
            pipe_operators: vec![],
        })),
        assignments: vec![],
        partitioned: None,
        after_columns: vec![],
        has_table_keyword: false,
        on: None,
        returning: None,
        replace_into: false,
        priority: None,
        insert_alias: None,
        settings: None,
        format_clause: None,
    }))
}

/// Extract properties from Cypher pattern
/// Example: "{name: 'Alice', age: 30}" → (["name", "age"], [Value('Alice'), Value(30)])
fn extract_properties(pattern: &str) -> Result<(Vec<Ident>, Vec<Expr>), String> {
    // Find the property map between { and }
    let start = pattern.find('{').ok_or("No properties found (missing '{')")?;
    let end = pattern.rfind('}').ok_or("No properties found (missing '}')")?;
    
    if start >= end {
        return Err("Invalid property syntax".to_string());
    }
    
    let props_str = &pattern[start + 1..end].trim();
    
    if props_str.is_empty() {
        return Ok((vec![], vec![]));
    }
    
    let mut columns = Vec::new();
    let mut values = Vec::new();
    
    // Simple parsing: split by comma, then by colon
    // This is simplified - a real implementation would use the tokenizer
    for pair in props_str.split(',') {
        let parts: Vec<&str> = pair.split(':').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            continue;
        }
        
        let key = parts[0].trim();
        let value = parts[1].trim();
        
        columns.push(Ident::new(key));
        
        // Parse the value
        let expr = parse_simple_value(value)?;
        values.push(expr);
    }
    
    Ok((columns, values))
}

/// Parse a simple value (string, number, boolean)
fn parse_simple_value(value: &str) -> Result<Expr, String> {
    let value = value.trim();
    
    // String literal (quoted)
    if (value.starts_with('\'') && value.ends_with('\'')) 
        || (value.starts_with('"') && value.ends_with('"')) {
        let unquoted = &value[1..value.len() - 1];
        return Ok(Expr::Value(Value::SingleQuotedString(unquoted.to_string()).into()));
    }
    
    // Boolean
    if value.eq_ignore_ascii_case("true") {
        return Ok(Expr::Value(Value::Boolean(true).into()));
    }
    if value.eq_ignore_ascii_case("false") {
        return Ok(Expr::Value(Value::Boolean(false).into()));
    }
    
    // Number
    if let Ok(num) = value.parse::<i64>() {
        return Ok(Expr::Value(Value::Number(num.to_string(), false).into()));
    }
    
    // Identifier (unquoted)
    Ok(Expr::Identifier(Ident::new(value)))
}

fn convert_return_items(return_items: &[SelectItem]) -> Vec<SelectItem> {
    return_items.iter().map(|item| {
        match item {
            SelectItem::UnnamedExpr(Expr::Identifier(_)) => {
                SelectItem::Wildcard(WildcardAdditionalOptions::default())
            }
            SelectItem::Wildcard(_) => item.clone(),
            _ => item.clone()
        
        }
    }).collect()
}

fn extract_first_label(pattern: &str) -> Result<String, String> {
    if let Some(colon_pos) = pattern.find(':') {
        let after_colon = &pattern[colon_pos + 1..];

        let after_colon = after_colon.trim_start();

        let label: String = after_colon
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        if label.is_empty() {
            Err("No label found after ':'".to_string())
        } else {
            Ok(label)
        }
    } else {
        Err("No label found in pattern (missing ':')".to_string())
    }
}

fn extract_first_variable(pattern: &str) -> Option<String> {
    if let Some(paren_pos) = pattern.find('(') {
        let after_paren = &pattern[paren_pos + 1..];

        let after_paren = after_paren.trim_start();

        let var: String = after_paren
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        if var.is_empty() {
            None
        } else {
            Some(var)
        }
    } else {
        None
    }
}

fn create_select(
    projection: Vec<SelectItem>,
    from: Vec<TableWithJoins>,
    selection: Option<Expr>,
) -> Select {
    Select {
        select_token: AttachedToken::empty(),
        distinct: None,
        top: None,
        top_before_distinct: false,
        projection,
        exclude: None,
        into: None,
        from,
        lateral_views: vec![],
        prewhere: None,
        selection,
        group_by: GroupByExpr::Expressions(vec![], vec![]),
        cluster_by: vec![],
        distribute_by: vec![],
        sort_by: vec![],
        having: None,
        named_window: vec![],
        qualify: None,
        window_before_qualify: false,
        value_table_mode: None,
        connect_by: None,
        flavor: SelectFlavor::Standard, 
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_first_label() {
        assert_eq!(extract_first_label("(n:Person)").unwrap(), "Person");
        assert_eq!(extract_first_label("(a:Company)").unwrap(), "Company");
        assert_eq!(extract_first_label("( n : Person )").unwrap(), "Person");
    }
    
    #[test]
    fn test_extract_first_variable() {
        assert_eq!(extract_first_variable("(n:Person)"), Some("n".to_string()));
        assert_eq!(extract_first_variable("(abc:Person)"), Some("abc".to_string()));
        assert_eq!(extract_first_variable("(:Person)"), None);
    }
    
    #[test]
    fn test_cypher_to_sql_simple() {
        let pattern = "(n:Person)";
        let where_clause = None;
        let return_items = vec![SelectItem::UnnamedExpr(
            Expr::CompoundIdentifier(vec![Ident::new("n"), Ident::new("name")])
        )];
        
        let result = cypher_to_sql(pattern, &where_clause, &return_items);
        assert!(result.is_ok());
        
        let sql_stmt = result.unwrap();
        let sql_str = sql_stmt.to_string();
        
        println!("Generated SQL: {}", sql_str);
        assert!(sql_str.contains("SELECT"));
        assert!(sql_str.contains("FROM"));
        assert!(sql_str.contains("Person"));
    }

    #[test]
    fn test_cypher_return_whole_node() {
        let pattern = "(n:Person)";
        let where_clause = None;
        // RETURN n (just the variable, not a property)
        let return_items = vec![SelectItem::UnnamedExpr(
            Expr::Identifier(Ident::new("n"))
        )];
        
        let result = cypher_to_sql(pattern, &where_clause, &return_items);
        assert!(result.is_ok());
        
        let sql_stmt = result.unwrap();
        let sql_str = sql_stmt.to_string();
        
        println!("Generated SQL for RETURN n: {}", sql_str);
        // Should be: SELECT * FROM Person AS n
        assert!(sql_str.contains("SELECT *") || sql_str.contains("SELECT*"));
        assert!(sql_str.contains("FROM Person"));
    }
    
    #[test]
    fn test_cypher_create_to_sql() {
        let pattern = "(n:Person {name: 'Alice', age: 30})";
        
        let result = cypher_create_to_sql(pattern);
        assert!(result.is_ok());
        
        let sql_stmt = result.unwrap();
        let sql_str = sql_stmt.to_string();
        
        println!("Generated INSERT SQL: {}", sql_str);
        assert!(sql_str.contains("INSERT"));
        assert!(sql_str.contains("Person"));
        assert!(sql_str.contains("name"));
        assert!(sql_str.contains("Alice"));
    }
}