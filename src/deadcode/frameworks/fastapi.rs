use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ruff_python_ast::{Expr, Stmt};

use crate::deadcode::symbols::{ImportTarget, ModuleSymbols, SymbolKey};

const ROUTE_DECORATORS: &[&str] = &[
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "options",
    "head",
    "api_route",
    "websocket",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectKind {
    App,
    Router,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct IncludeEdge {
    parent: SymbolKey,
    child: SymbolKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RouteEdge {
    router: SymbolKey,
    handler: SymbolKey,
}

#[derive(Debug, Clone, Default)]
pub(in crate::deadcode) struct ModuleInfo {
    objects: BTreeMap<SymbolKey, ObjectKind>,
    includes: BTreeSet<IncludeEdge>,
    routes: BTreeSet<RouteEdge>,
}

pub(in crate::deadcode) fn collect_module_info(
    module_path: &str,
    body: &[Stmt],
    imports: &BTreeMap<String, ImportTarget>,
    local_symbols: &BTreeSet<String>,
) -> ModuleInfo {
    let mut info = ModuleInfo::default();

    for stmt in body {
        collect_object_assignment(module_path, stmt, imports, &mut info);
        collect_include_edge(module_path, stmt, imports, local_symbols, &mut info);
        collect_route_edge(module_path, stmt, imports, local_symbols, &mut info);
    }

    info
}

pub(in crate::deadcode) fn live_symbols(
    modules: &BTreeMap<String, ModuleSymbols>,
    root_symbols: &BTreeSet<SymbolKey>,
) -> BTreeSet<SymbolKey> {
    let mut objects = BTreeMap::<SymbolKey, ObjectKind>::new();
    let mut includes = BTreeMap::<SymbolKey, BTreeSet<SymbolKey>>::new();
    let mut routes = BTreeMap::<SymbolKey, BTreeSet<SymbolKey>>::new();

    for module in modules.values() {
        objects.extend(
            module
                .fastapi
                .objects
                .iter()
                .map(|(key, kind)| (key.clone(), *kind)),
        );
        for include in &module.fastapi.includes {
            includes
                .entry(include.parent.clone())
                .or_default()
                .insert(include.child.clone());
        }
        for route in &module.fastapi.routes {
            routes
                .entry(route.router.clone())
                .or_default()
                .insert(route.handler.clone());
        }
    }

    let mut live = BTreeSet::<SymbolKey>::new();
    let mut queue = VecDeque::<SymbolKey>::new();

    for root in root_symbols {
        if objects.contains_key(root) && live.insert(root.clone()) {
            queue.push_back(root.clone());
        }
    }

    while let Some(current) = queue.pop_front() {
        if let Some(children) = includes.get(&current) {
            for child in children {
                if objects.contains_key(child) && live.insert(child.clone()) {
                    queue.push_back(child.clone());
                }
            }
        }
    }

    for object in live.clone() {
        if let Some(handlers) = routes.get(&object) {
            live.extend(handlers.iter().cloned());
        }
    }

    live
}

fn collect_object_assignment(
    module_path: &str,
    stmt: &Stmt,
    imports: &BTreeMap<String, ImportTarget>,
    info: &mut ModuleInfo,
) {
    match stmt {
        Stmt::Assign(assign) => {
            let Some(kind) = fastapi_constructor_kind(&assign.value, imports) else {
                return;
            };
            for target in &assign.targets {
                if let Some(name) = simple_name(target) {
                    info.objects
                        .insert(SymbolKey::new(module_path, name.to_string()), kind);
                }
            }
        }
        Stmt::AnnAssign(assign) => {
            let Some(value) = &assign.value else {
                return;
            };
            let Some(kind) = fastapi_constructor_kind(value, imports) else {
                return;
            };
            if assign.simple
                && let Some(name) = simple_name(&assign.target)
            {
                info.objects
                    .insert(SymbolKey::new(module_path, name.to_string()), kind);
            }
        }
        _ => {}
    }
}

fn collect_include_edge(
    module_path: &str,
    stmt: &Stmt,
    imports: &BTreeMap<String, ImportTarget>,
    local_symbols: &BTreeSet<String>,
    info: &mut ModuleInfo,
) {
    let Some(expr) = statement_expression(stmt) else {
        return;
    };
    let Some((parent, child)) = include_router_edge(expr, module_path, imports, local_symbols)
    else {
        return;
    };
    info.includes.insert(IncludeEdge { parent, child });
}

fn collect_route_edge(
    module_path: &str,
    stmt: &Stmt,
    imports: &BTreeMap<String, ImportTarget>,
    local_symbols: &BTreeSet<String>,
    info: &mut ModuleInfo,
) {
    let Stmt::FunctionDef(function) = stmt else {
        return;
    };
    let handler = SymbolKey::new(module_path, function.name.to_string());

    for decorator in &function.decorator_list {
        if let Some(router) =
            route_decorator_object(&decorator.expression, module_path, imports, local_symbols)
        {
            info.routes.insert(RouteEdge {
                router,
                handler: handler.clone(),
            });
        }
    }
}

fn statement_expression(stmt: &Stmt) -> Option<&Expr> {
    match stmt {
        Stmt::Expr(stmt) => Some(&stmt.value),
        Stmt::Assign(assign) => Some(&assign.value),
        Stmt::AnnAssign(assign) => assign.value.as_deref(),
        _ => None,
    }
}

fn fastapi_constructor_kind(
    expr: &Expr,
    imports: &BTreeMap<String, ImportTarget>,
) -> Option<ObjectKind> {
    let Expr::Call(call) = expr else {
        return None;
    };

    match expression_path(&call.func, imports).as_deref() {
        Some("fastapi.FastAPI") => Some(ObjectKind::App),
        Some("fastapi.APIRouter") => Some(ObjectKind::Router),
        _ => None,
    }
}

fn include_router_edge(
    expr: &Expr,
    module_path: &str,
    imports: &BTreeMap<String, ImportTarget>,
    local_symbols: &BTreeSet<String>,
) -> Option<(SymbolKey, SymbolKey)> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Attribute(attribute) = &*call.func else {
        return None;
    };
    if attribute.attr.as_str() != "include_router" {
        return None;
    }

    let parent = resolve_symbol_expression(&attribute.value, module_path, imports, local_symbols)?;
    let child_expr = call.arguments.args.first().or_else(|| {
        call.arguments
            .keywords
            .iter()
            .find(|keyword| {
                keyword
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.as_str() == "router")
            })
            .map(|keyword| &keyword.value)
    })?;
    let child = resolve_symbol_expression(child_expr, module_path, imports, local_symbols)?;

    Some((parent, child))
}

fn route_decorator_object(
    expr: &Expr,
    module_path: &str,
    imports: &BTreeMap<String, ImportTarget>,
    local_symbols: &BTreeSet<String>,
) -> Option<SymbolKey> {
    let decorator_func = match expr {
        Expr::Call(call) => &*call.func,
        other => other,
    };
    let Expr::Attribute(attribute) = decorator_func else {
        return None;
    };
    if !ROUTE_DECORATORS.contains(&attribute.attr.as_str()) {
        return None;
    }

    resolve_symbol_expression(&attribute.value, module_path, imports, local_symbols)
}

fn resolve_symbol_expression(
    expr: &Expr,
    module_path: &str,
    imports: &BTreeMap<String, ImportTarget>,
    local_symbols: &BTreeSet<String>,
) -> Option<SymbolKey> {
    match expr {
        Expr::Name(name) => {
            if local_symbols.contains(name.id.as_str()) {
                return Some(SymbolKey::new(module_path, name.id.to_string()));
            }
            match imports.get(name.id.as_str()) {
                Some(ImportTarget::Symbol(symbol)) => Some(symbol.clone()),
                _ => None,
            }
        }
        Expr::Attribute(attribute) => resolve_module_expression(&attribute.value, imports)
            .map(|module_path| SymbolKey::new(module_path, attribute.attr.to_string())),
        _ => None,
    }
}

fn expression_path(expr: &Expr, imports: &BTreeMap<String, ImportTarget>) -> Option<String> {
    match expr {
        Expr::Name(name) => match imports.get(name.id.as_str()) {
            Some(ImportTarget::Module(module_path)) => Some(module_path.clone()),
            Some(ImportTarget::Symbol(symbol)) => {
                Some(format!("{}.{}", symbol.module_path, symbol.symbol_name))
            }
            None => Some(name.id.to_string()),
        },
        Expr::Attribute(attribute) => resolve_module_expression(&attribute.value, imports)
            .map(|module_path| format!("{}.{}", module_path, attribute.attr))
            .or_else(|| {
                expression_path(&attribute.value, imports)
                    .map(|prefix| format!("{}.{}", prefix, attribute.attr))
            }),
        _ => None,
    }
}

fn resolve_module_expression(
    expr: &Expr,
    imports: &BTreeMap<String, ImportTarget>,
) -> Option<String> {
    match expr {
        Expr::Name(name) => imports
            .get(name.id.as_str())
            .and_then(|target| match target {
                ImportTarget::Module(module_path) => Some(module_path.clone()),
                ImportTarget::Symbol(_) => None,
            }),
        Expr::Attribute(attribute) => resolve_module_expression(&attribute.value, imports)
            .map(|prefix| format!("{}.{}", prefix, attribute.attr)),
        _ => None,
    }
}

fn simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}
