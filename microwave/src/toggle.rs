use std::slice;

#[derive(Clone)]
pub struct Toggle<T> {
    options: Vec<T>,
    curr_index: usize,
}

impl<T> Toggle<T> {
    pub fn with_initial_index(options: Vec<T>, index: usize) -> Self {
        assert!(
            index < options.len(),
            "index {index} is out of bounds for {} options",
            options.len()
        );
        Toggle {
            options,
            curr_index: index,
        }
    }

    pub fn curr_index(&self) -> usize {
        self.curr_index
    }

    pub fn num_options(&self) -> usize {
        self.options.len()
    }

    pub fn toggle_next(&mut self) {
        self.curr_index = (self.curr_index + 1) % self.options.len();
    }

    pub fn inc(&mut self) {
        self.curr_index = (self.curr_index + 1).min(self.options.len() - 1);
    }

    pub fn dec(&mut self) {
        self.curr_index = self.curr_index.saturating_sub(1);
    }

    pub fn curr_option(&self) -> &T {
        &self.options[self.curr_index]
    }

    pub fn curr_option_mut(&mut self) -> &mut T {
        &mut self.options[self.curr_index]
    }
}

impl<'a, T> IntoIterator for &'a mut Toggle<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.options.iter_mut()
    }
}

impl<T> From<Vec<T>> for Toggle<T> {
    fn from(options: Vec<T>) -> Self {
        Toggle::with_initial_index(options, 0)
    }
}
