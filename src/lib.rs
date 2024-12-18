use wasmcloud_component::{http::{self, ErrorCode, IncomingBody, OutgoingBody, Request, Response, Server}, info};
use serde::{Deserialize, Serialize};
use std::{sync::{Arc, Mutex}, thread::sleep, time::Duration};
use uuid::Uuid;
use std::io::Read;
use lazy_static::lazy_static;

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

#[derive(Serialize, Clone)]
struct WorkflowExecution {
    id: String,
    name: String,
    status: String,
}

struct AppState {
    executions: Mutex<Vec<WorkflowExecution>>, 
}

// Utilisation de `lazy_static` pour un état global
lazy_static! {
    static ref APP_STATE: Arc<AppState> = Arc::new(AppState {
        executions: Mutex::new(Vec::new()),
    });
}

fn create_workflow(workflow: Workflow, app_state: &AppState) -> String {
    let workflow_id = Uuid::new_v4();

    let execution = WorkflowExecution {
        id: workflow_id.to_string(),
        name: workflow.name.clone(),
        status: "pending".to_string(),
    };

    {
        let mut executions = app_state.executions.lock().unwrap();
        executions.push(execution.clone());
    }

    for task in workflow.task {
        match task.tasktype.as_str() {
            "add" => {
                let numbers: Vec<i32> = serde_json::from_value(task.value).unwrap();
                let sum: i32 = numbers.iter().sum();
                info!("Addition résultat: {}", sum);
            }
            "print" => {
                let text: String = serde_json::from_value(task.value).unwrap();
                info!("Message à imprimer: {}", text);
            }
            "wait" => {
                let seconds: u64 = serde_json::from_value(task.value).unwrap();
                sleep(Duration::from_secs(seconds));
                info!("Attente de {} secondes", seconds);
            }
            _ => {
                info!("Tâche inconnue: {}", task.tasktype);
            }
        }
    }

    {
        let mut executions = app_state.executions.lock().unwrap();
        if let Some(ex) = executions.iter_mut().find(|e| e.id == execution.id) {
            ex.status = "completed".to_string();
        }
    }

    format!(
        "Workflow {} exécuté avec succès",
        workflow_id
    )
}

fn get_executions(app_state: &AppState) -> Vec<WorkflowExecution> {
    let executions = app_state.executions.lock().unwrap();
    executions.clone()
}

fn get_workflow(path: &str, app_state: &AppState) -> Option<WorkflowExecution> {
    let path_parts: Vec<&str> = path.split('/').collect();
    let id = path_parts.last()?; 

    let executions = app_state.executions.lock().unwrap();
    executions.iter().find(|e| e.id == *id).cloned()
}

fn delete_workflow(path: &str, app_state: &AppState) -> Option<String> {
    let path_parts: Vec<&str> = path.split('/').collect();
    let id = path_parts.last()?; 

    let mut executions = app_state.executions.lock().unwrap();

    if let Some(pos) = executions.iter().position(|e| e.id == *id) {
        let removed_execution = executions.remove(pos);
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
                            .body("Invalid JSON".to_string())
                            .unwrap());
                    }
                };

                let result = create_workflow(body, &APP_STATE);
                Ok(http::Response::builder()
                    .status(200)
                    .body(result)
                    .unwrap())
            }
            ("GET", path) if path.starts_with("/workflow/") => {
                match get_workflow(path, &APP_STATE) {
                    Some(workflow) => {
                        let body = serde_json::to_string(&workflow).unwrap();
                        Ok(http::Response::builder()
                            .status(200)
                            .body(body)
                            .unwrap())
                    }
                    None => Ok(http::Response::builder()
                        .status(404)
                        .body("Workflow not found".to_string())
                        .unwrap()),
                }
            }
            ("DELETE", path) if path.starts_with("/workflow/") => {
                match delete_workflow(path, &APP_STATE) {
                    Some(message) => Ok(http::Response::builder()
                        .status(200)
                        .body(message)
                        .unwrap()),
                    None => Ok(http::Response::builder()
                        .status(404)
                        .body("Workflow not found".to_string())
                        .unwrap()),
                }
            }
            ("GET", "/workflows") => {
                let executions = get_executions(&APP_STATE);
                let result = serde_json::to_string(&executions).unwrap();
                Ok(http::Response::builder()
                    .status(200)
                    .body(result)
                    .unwrap())
            }
            _ => {
                Ok(http::Response::builder()
                    .status(404)
                    .body("Not found".to_string())
                    .unwrap())
            }
        }
    }
}
