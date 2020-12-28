/**
 * The Clemson Kilonova Code
 */




// ============================================================================
static DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
static VERSION_AND_BUILD: &str = git_version::git_version!(prefix=concat!("v", env!("CARGO_PKG_VERSION"), " "));




// ============================================================================
mod mesh;
mod models;
mod physics;
mod products;
mod scheme;
mod state;
mod tasks;
mod traits;




// ============================================================================
use std::{
    fs::{
        File,
        read_to_string,
    },
    path::Path,
    ffi::OsStr,
};
use serde::{
    Serialize,
    Deserialize,
};
use enum_dispatch::{
    enum_dispatch,
};
use mesh::{
    Mesh,
};
use models::{
    JetInCloud,
    HaloKilonova,
};
use physics::{
    AgnosticPrimitive,
    RelativisticHydro,
};
use products::{
    Products,
};
use state::{
    State,
};
use traits::{
    Conserved,
    Hydrodynamics,
    InitialModel,
};
use tasks::{
    Tasks,
};




/**
 * Model choice
 */
#[enum_dispatch(InitialModel)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "setup")]
pub enum Model {
    JetInCloud(JetInCloud),
    HaloKilonova(HaloKilonova),
}


/**
 * Enum for any of the supported hydrodynamics types
 */
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "system")]
pub enum AgnosticHydro {
    Euler,
    Relativistic(RelativisticHydro),
}


/**
 * Enum for the solution state of any of the supported hydrodynamics types
 */
#[derive(Clone, Serialize, Deserialize)]
pub enum AgnosticState {
    Euler,
    Relativistic(State<hydro_srhd::srhd_2d::Conserved>),
}


/**
 * Simulation control: how long to run for, how frequently to perform side
 * effects, etc.
 */
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub final_time: f64,
    pub checkpoint_interval: f64,
    pub products_interval: f64,
}


/**
 * User configuration
 */
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub hydro: AgnosticHydro,
    pub model: Model,
    pub mesh: Mesh,
    pub control: Control,
}


/**
 * App state
 */
#[derive(Clone, Serialize, Deserialize)]
pub struct App {
    state: AgnosticState,
    tasks: Tasks,
    config: Configuration,
}




// ============================================================================
impl From<State<hydro_srhd::srhd_2d::Conserved>> for AgnosticState {
    fn from(state: State<hydro_srhd::srhd_2d::Conserved>) -> Self {
        Self::Relativistic(state)
    }
}

impl From<RelativisticHydro> for AgnosticHydro {
    fn from(hydro: RelativisticHydro) -> Self {
        Self::Relativistic(hydro)
    }
}




// ============================================================================
impl Configuration {
    fn package<H>(hydro: &H, model: &Model, mesh: &Mesh, control: &Control) -> Self
    where
        H: Hydrodynamics,
        AgnosticHydro: From<H> {
        Configuration{
            hydro: AgnosticHydro::from(hydro.clone()),
            model: model.clone(),
            mesh: mesh.clone(),
            control: control.clone(),
        }
    }
}




// ============================================================================
impl App {

    /**
     * Construct a new App instance from a user configuration
     */
    fn from_config(config: Configuration) -> anyhow::Result<Self> {
        let geometry = config.mesh.grid_blocks_geometry()?;
        let state = match &config.hydro {
            AgnosticHydro::Euler => {
                anyhow::bail!("hydro: euler is not implemented yet")
            },
            AgnosticHydro::Relativistic(hydro) => {
                AgnosticState::from(State::from_model(&config.model, hydro, &geometry))
            },
        };
        let tasks = Tasks::new();
        Ok(Self{state, tasks, config})
    }

    /**
     * Construct a new App instance from the command line arguments
     */
    fn build() -> anyhow::Result<Self> {
        match std::env::args().skip(1).next() {
            None => anyhow::bail!("no input file given"),
            Some(input_file) => match Path::new(&input_file).extension().and_then(OsStr::to_str) {
                Some("yaml") => Self::from_config(serde_yaml::from_reader(File::open(input_file)?)?),
                Some("toml") => Self::from_config(toml::from_str(&read_to_string(input_file)?)?),
                Some("pk") => Ok(serde_pickle::from_reader(File::open(input_file)?)?),
                _ => anyhow::bail!("unknown input file type '{}'", input_file),
            }
        }
    }

    fn package<C, H>(state: &State<C>, tasks: &mut Tasks, hydro: &H, model: &Model, mesh: &Mesh, control: &Control) -> Self
    where
        H: Hydrodynamics<Conserved = C>,
        C: Conserved,
        AgnosticState: From<State<C>>,
        AgnosticHydro: From<H> {
        Self{
            state: AgnosticState::from(state.clone()),
            tasks: tasks.clone(),
            config: Configuration::package(hydro, model, mesh, control),
        }
    }
}




// ============================================================================
fn side_effects<C, H>(state: &State<C>, tasks: &mut Tasks, hydro: &H, model: &Model, mesh: &Mesh, control: &Control)
    -> anyhow::Result<()>
where
    H: Hydrodynamics<Conserved = C>,
    C: Conserved,
    AgnosticState: From<State<C>>,
    AgnosticHydro: From<H> {

    if tasks.iteration_message.next_time <= state.time {
        let time = tasks.iteration_message.advance(0.0);
        let mzps = 1e-6 * state.total_zones() as f64 / time;
        if tasks.iteration_message.count_this_run > 1 {
            println!("[{:05}] t={:.3} blocks={} Mzps={:.2})", state.iteration, state.time, state.solution.len(), mzps);
        }
    }

    if tasks.write_checkpoint.next_time <= state.time {
        tasks.write_checkpoint.advance(control.checkpoint_interval);
        let filename = format!("chkpt.{:04}.pk", tasks.write_checkpoint.count - 1);
        let app = App::package(state, tasks, hydro, model, mesh, control);
        let mut buffer = std::io::BufWriter::new(File::create(&filename)?);
        println!("write {}", filename);
        serde_pickle::to_writer(&mut buffer, &app, true)?;
    }

    if tasks.write_products.next_time <= state.time {
        tasks.write_products.advance(control.products_interval);
        let filename = format!("prods.{:04}.pk", tasks.write_products.count - 1);
        let config = Configuration::package(hydro, model, mesh, control);
        let products = Products::from_state(state, hydro, mesh, &config)?;
        let mut buffer = std::io::BufWriter::new(File::create(&filename)?);
        println!("write {}", filename);
        serde_pickle::to_writer(&mut buffer, &products, true)?;
    }

    Ok(())
}




// ============================================================================
fn run<C, H>(mut state: State<C>, mut tasks: Tasks, hydro: H, model: Model, mesh: Mesh, control: Control)
    -> anyhow::Result<()>
where
    H: Hydrodynamics<Conserved = C>,
    C: Conserved,
    AgnosticState: From<State<C>>,
    AgnosticHydro: From<H> {

    while state.time < control.final_time {
        side_effects(&state, &mut tasks, &hydro, &model, &mesh, &control)?;
        scheme::advance(&mut state, &hydro, &mesh);
    }

    side_effects(&state, &mut tasks, &hydro, &model, &mesh, &control)?;

    Ok(())
}




// ============================================================================
fn main() -> anyhow::Result<()> {

    let App{state, tasks, config} = App::build()?;
    let Configuration{hydro, model, mesh, control} = config;

    println!("{}", DESCRIPTION);
    println!("{}", VERSION_AND_BUILD);

    match (state, hydro) {
        (AgnosticState::Euler, _) => {
            anyhow::bail!("Euler hydrodynamics not implemented")
        },
        (AgnosticState::Relativistic(state), AgnosticHydro::Relativistic(hydro)) => {
            run(state, tasks, hydro, model, mesh, control)
        },
        _ => unreachable!(),
    }
}
