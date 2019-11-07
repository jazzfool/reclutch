use crate::traits::Listen;

pub type Listener<T> = Vec<Box<dyn Merge<T>>>;

pub trait Merge<T> {
    fn extend_other(&self, o: &mut Vec<T>);
    fn indirect_with(&self, f: &mut dyn FnMut(&T));
}

impl<T, EL> Merge<T> for EL
where
    T: Clone,
    EL: Listen<T>,
{
    fn extend_other(&self, o: &mut Vec<T>) {
        self.with(|j| o.extend(j.iter().cloned()));
    }
    fn indirect_with(&self, f: &mut dyn FnMut(&T)) {
        self.with(|j| {
            for i in j.iter() {
                (*f)(i);
            }
        });
    }
}

impl<T> Listen<T> for Listener<T> {
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        let mut events = Vec::<T>::new();
        for i in self.iter() {
            i.extend_other(&mut events);
        }
        f(&events[..])
    }

    fn map<F, R>(&self, mut f: F) -> Vec<R>
    where
        F: FnMut(&T) -> R,
    {
        let mut ret = Vec::new();
        for i in self.iter() {
            i.indirect_with(&mut |j| ret.push(f(j)));
        }
        ret
    }
}
