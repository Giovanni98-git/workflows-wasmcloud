wit_bindgen::generate!({ generate_all }); //  génère automatiquement des bindings Rust pour un monde WIT
use wasmcloud_component::wasi::keyvalue::*;
use wasmcloud_component::{http::{self, ErrorCode, IncomingBody, OutgoingBody, Request, Response, Server}, info};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::io::Read;

use example::{add::adder::add, print::printer::print as printer, wait::waiter::wait};

#[derive(Deserialize)]
struct Task {
    tasktype: String,
    value: serde_json::Value,
}

#[derive(Deserialize)]
struct Workflow {
    name: String,
    task: Vec<Task>,
}

#[derive(Debug, Serialize, Clone, serde::Deserialize)]
struct WorkflowExecution {
    id: String,
    name: String,
    status: String,
}

fn get_workflows() -> Vec<WorkflowExecution> {
    let bucket = store::open("default").unwrap();
    let storage: Option<Vec<u8>> = bucket.get("workflows").unwrap();
    drop(bucket);
    
    if let Some(data) = storage {
        match serde_json::from_slice::<Vec<WorkflowExecution>>(&data) {
            Ok(workflows) => workflows, 
            Err(_) => {
                Vec::new()
            }
        }
    } else {
        Vec::new()
    }
}

fn save_worklfows( workflows :Vec<WorkflowExecution>){
    let bucket = store::open("default").unwrap();
    let workflows = serde_json::to_string(&workflows).unwrap();
    let workflows_bytes : Vec<u8> = workflows.into_bytes();
    let _ = bucket.set("workflows", &workflows_bytes);
    drop(bucket);
}

fn create_workflow(workflow: Workflow) -> String {
    let workflow_id = Uuid::new_v4();

    let execution = WorkflowExecution {
        id: workflow_id.to_string(),
        name: workflow.name.clone(),
        status: "pending".to_string(),
    };

    let mut executions = get_workflows();
        executions.push(execution.clone());
        save_worklfows(executions);

    for task in workflow.task {
        match task.tasktype.as_str() {
            "add" => {
                let numbers: Vec<i32> = serde_json::from_value(task.value).unwrap();
                let paste_numbers = serde_json::to_string(&numbers).unwrap();
                let response = add(&paste_numbers);
                info!("{:?}", response);
            }
            "print" => {
                let text: String = serde_json::from_value(task.value).unwrap();
                let message = printer(&text);
                info!("Message: {}", message);
            }
            "wait" => {
                let seconds: u64 = serde_json::from_value(task.value).unwrap();
                let response = wait(&serde_json::to_string(&seconds).unwrap());
                info!("{:?}", response);
            }
            _ => {
                info!("Tâche inconnue: {}", task.tasktype);
               return  format!("Workflow non exécuté avec succès")
            }
        }
    }

    let mut executions = get_workflows();
    if let Some(pos) = executions.iter_mut().find(|e| e.id == workflow_id.to_string()) {
        pos.status = "completed".to_string();
        save_worklfows(executions);
    } 

    format!( "Workflow {} exécuté avec succès", workflow_id)
}

fn get_workflow(path: &str) -> Option<WorkflowExecution> {
    let path_parts: Vec<&str> = path.split('/').collect();
    let id = path_parts.last()?; 

    let executions = get_workflows();
    executions.iter().find(|e| e.id == *id).cloned()
}

fn delete_workflow(path: &str) -> Option<String> {
    let path_parts: Vec<&str> = path.split('/').collect();
    let id = path_parts.last()?; 

    let mut executions = get_workflows();

    if let Some(pos) = executions.iter().position(|e| e.id == *id) {
        let removed_execution = executions.remove(pos);
        save_worklfows(executions);
        return Some(format!("Workflow {} supprimé avec succès", removed_execution.id));
    }
    None
}

struct Component;

http::export!(Component);

impl Server for Component {
    fn handle(
        request: Request<IncomingBody>,
    ) -> Result<Response<impl OutgoingBody>, ErrorCode> {

        match (request.method().as_str(), request.uri().path()) {
            ("POST", "/workflows") => {
                let (_parts, mut body) = request.into_parts();
                let mut body_bytes = Vec::new();
                body.read_to_end(&mut body_bytes).map_err(|_| ErrorCode::InternalError(Some("Invalid JSON".to_string())))?;

                let body: Workflow = match serde_json::from_slice(&body_bytes) {
                    Ok(data) => data,
                    Err(_) => {
                        return Ok(http::Response::builder()
                            .status(400)
                            .header("Content-Type", "application/json")
                            .body("Invalid JSON".to_string())
                            .unwrap());
                    }
                };

                let result = create_workflow(body);
                Ok(http::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(result)
                    .unwrap())
            }
            ("GET", path) if path.starts_with("/workflows/") => {
                match get_workflow(path) {
                    Some(workflow) => {
                        let body = serde_json::to_string(&workflow).unwrap();
                        Ok(http::Response::builder()
                            .status(200)
                            .header("Content-Type", "application/json")
                            .body(body)
                            .unwrap())
                    }
                    None => Ok(http::Response::builder()
                        .status(404)
                        .header("Content-Type", "application/json")
                        .body("Workflow not found".to_string())
                        .unwrap()),
                }
            }
            ("DELETE", path) if path.starts_with("/workflows/") => {
                match delete_workflow(path) {
                    Some(message) => Ok(http::Response::builder()
                        .status(200)
                        .header("Content-Type", "application/json")
                        .body(message)
                        .unwrap()),
                    None => Ok(http::Response::builder()
                        .status(404)
                        .header("Content-Type", "application/json")
                        .body("Workflow not found".to_string())
                        .unwrap()),
                }
            }
            ("GET", "/workflows") => {
                let executions = get_workflows();
                let result = serde_json::to_string(&executions).unwrap();
                Ok(http::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(result)
                    .unwrap())
            }
            _ => {
                Ok(http::Response::builder()
                    .status(404)
                    .header("Content-Type", "application/json")
                    .body("Not found".to_string())
                    .unwrap())
            }
        }
    }
}