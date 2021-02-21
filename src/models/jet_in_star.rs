use std::f64::consts::PI;
use serde::{Serialize, Deserialize};
use crate::physics::{AnyPrimitive, LIGHT_SPEED};
use crate::traits::InitialModel;




static UNIFORM_TEMPERATURE: f64 = 1e-3;

// Constants as given in Duffel & MacDayen(2015)
// source: https://arxiv.org/pdf/1407.8250.pdf
static R0:                  f64 = 7e10;
static M0:                  f64 = 2e33;
static RHO_C:               f64 = 3e7 * M0 / (1.33 * PI * R0 * R0 * R0);
static R1:                  f64 = 0.0017 * R0;
static R2:                  f64 = 0.0125 * R0;
static R3:                  f64 = 0.65   * R0;
static K1:                  f64 = 3.24;
static K2:                  f64 = 2.57;
static N:                   f64 = 16.7;
static RHO_WIND:            f64 = 1e-9 * M0 / (1.33 * PI * R0 * R0 * R0);
static RHO_ENV:             f64 = 1e-7 * M0 / (1.33 * PI * R0 * R0 * R0);
static R_NOZZ:              f64 = 0.01 * R0; 
static R_ENV:               f64 = 1.2  * R0;




/**
 * Jet propagating through a star and surrounding relativistic
 * envelope
 */
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JetInStar {

    /// Mass of the star
    pub star_mass: f64,

    /// Duration of the engine
    pub engine_duration: f64,

    /// E is the isotropic equivalent of energy in cgs
    pub engine_energy: f64,

    /// Engine opening angle
    pub engine_theta: f64,

    /// Engine four-velocity
    pub engine_u: f64,
}




/**
 * Different space-time zones in the setup
 */
pub enum Zone {
    Core,
    Envelope,
    Wind,
    Jet,
}




// ============================================================================
impl InitialModel for JetInStar {

    fn validate(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn primitive_at(&self, coordinate: (f64, f64), t: f64) -> AnyPrimitive {
        let (r, q) = coordinate;
        let d = self.mass_density(r, q, t);
        let u = self.gamma_beta(r, q, t);
        let p = d * UNIFORM_TEMPERATURE;

        AnyPrimitive {
            velocity_r: u,
            velocity_q: 0.0,
            mass_density: d,
            gas_pressure: p,
        }
    }

    fn scalar_at(&self, coordinate: (f64, f64), t: f64) -> f64 {
        let (r, q) = coordinate;
        let zone   = self.zone(r, q, t);

        match zone {
            Zone::Core     => 1e+0,
            Zone::Jet      => 1e+2,
            Zone::Envelope => 1e-2 * (r / R3).powf(-2.0),
            Zone::Wind     => 1e-5 * (r / R_ENV).powf(-2.0),
        }
    }
}




// ============================================================================
impl JetInStar
{
    /**
     * The comoving mass density in g/cc
     */
    fn mass_density(&self, r: f64, q: f64, t: f64) -> f64{
        let zone      = self.zone(r, q, t);
        let num       = RHO_C * ((1.0 - r / R3)).powf(N);
        let denom     = 1.0 + (r / R1).powf(K1) / (1.0 + (r / R2).powf(K2));
        let core_zone = num/denom;

        match zone {
            Zone::Core     => core_zone + RHO_ENV * (r/R3).powf(-2.0),
            Zone::Envelope => RHO_ENV*(r/R3).powf(-2.0),
            Zone::Jet      => self.jet_mass_rate_per_steradian(r, q) / (r * r * self.engine_u * LIGHT_SPEED),
            Zone::Wind     => RHO_WIND * (r/R_ENV).powf(-2.0),
        }
    }

    /**
     * Dimensionless jet velocity: v_jet / c
     */
    pub fn engine_beta(&self) -> f64 {
        self.engine_u / (1.0 + self.engine_u.powi(2)).sqrt()
    }

    /**
     * Determine if a polar angle is within theta_jet of either pole.
     *
     * * `q` - The polar angle theta
     */
    pub fn in_nozzle(&self, q: f64) -> bool {
        q < self.engine_theta || q > PI - self.engine_theta
    }

    /**
     * Determine the location of the jet head
     *
     * * `t` - Time
     */
    pub fn get_jet_head(&self, t: f64) -> f64 {
        let v_jet = self.engine_beta() * LIGHT_SPEED;
        v_jet * t
    }

    /**
     * Determine the zone of the ambient medium for a given radius and time.
     *
     * * `r` - Radius
     * * `q` - Polar angle
     * * `t` - Time
     */
    pub fn zone(&self, r: f64, q: f64, t: f64) -> Zone {
        let v_jet = self.engine_beta() * LIGHT_SPEED;
        let r_jet_head = v_jet * t;

        if self.in_nozzle(q) && r < r_jet_head {
            Zone::Jet
        } else if r < R3 {
            Zone::Core
        } else if R3 < r && r < R_ENV {
            Zone:: Envelope
        } else {
            Zone::Wind
        }
    }

    /**
     * Return the radial four-velocity (gamma-beta).
     *
     * * `r` - The radius
     * * `q` - The polar angle theta
     * * `t` - The time
     */
    pub fn gamma_beta(&self, r: f64, q: f64, t: f64) -> f64 {
        match self.zone(r, q, t) {
            Zone::Jet => {
                self.engine_u
            }
            _ => {
                0.0
            }

        }
    }

    /**
     * Return the fictitious nozzle function as described in
     * Duffel & MAcFadyen (2015)
     * 
     * * `r' - The radius
     * * `q` - The polar angle theta
     */
    pub fn nozzle_function(&self, r: f64, q: f64) -> f64 {
        // Normalize the Nozzle Radius
        let r0 = R_NOZZ/R0;
        let q2 = self.engine_theta.powi(2);

        // Nozzle Function Normalization Factor
        // N0 = 4 * PI * r0^3 * exp(-2/theta0^2) * theta0^2
        let n_0 =  4.0 * PI * r0 * r0 * r0 * (1. - (-2.0 / q2).exp()) * q2;

        // Nozzle Function: g = (r/r0) * exp(-(r/r0)^2) * exp[(cos^2(q) - 1 )/theta0^2] / N0
        let g = (r / R_NOZZ) * f64::exp(-(r / R_NOZZ).powf(2.0) / 2.0) * f64::exp((q.cos().powf(2.0) - 1.0) / q2);

        g / n_0
    }

    fn jet_mass_rate_per_steradian(&self, r: f64, q: f64) -> f64 {
        let engine_gamma = f64::sqrt(1.0 + self.engine_u * self.engine_u);
        let e = self.engine_energy;
        let l = self.nozzle_function(r, q) * e / (4.0 * PI * self.engine_duration);
        l / (engine_gamma * LIGHT_SPEED * LIGHT_SPEED)
    }
}




// Custom Pause Function
// use std::io::{stdin, stdout, Read, Write};
// fn pause() {
//     let mut stdout = stdout();
//     stdout.write(b"Press Enter to continue...").unwrap();
//     stdout.flush().unwrap();
//     stdin().read(&mut [0]).unwrap();
// }
