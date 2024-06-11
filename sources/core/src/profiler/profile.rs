//! ...

use smol_str::SmolStr;

#[derive(Clone, Debug, Default)]
pub struct Profile {
  pub name: SmolStr,
  pub time: f64,
}

impl std::fmt::Display for Profile {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
      f,
      "{{
      \"name\": \"{}\",
      \"time\": {}
     }}",
      self.name, self.time
    )
  }
}

#[derive(Clone, Debug, Default)]
pub struct Profiles(pub Vec<Profile>);

impl Profiles {
  #[inline]
  pub fn add_profile(&mut self, profile: Profile) {
    self.0.push(profile);
  }

  #[inline]
  pub fn total(&self) -> f64 {
    self.iter().map(|profile| profile.time).sum()
  }
}

impl std::ops::Deref for Profiles {
  type Target = Vec<Profile>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::fmt::Display for Profiles {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    self
      .iter()
      .try_fold((), |_, profile| write!(f, "{profile}"))
  }
}
