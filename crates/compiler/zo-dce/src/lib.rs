mod dce;

pub use dce::eliminate_dead_functions;

#[cfg(test)]
mod tests;
