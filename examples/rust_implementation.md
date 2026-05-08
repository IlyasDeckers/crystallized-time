# Rust Implementation
Notes, references and snippets about implementing a quantum state simulator in Rust.

## Complex numbers

```rust
x = a + bi

i^2 = -1

  struct ComplexNum {
    real: f64,
    imaginary: f64,
  }
```

```rust
// Creating complex numbers
fn main() {
    let complex_integer = num::complex::Complex::new(10, 20);
    let complex_float = num::complex::Complex::new(10.1, 20.1);

    println!("Complex integer: {}", complex_integer);
    println!("Complex float: {}", complex_float);
}

// Adding complex numbers
// Performing mathematical operations on complex numbers is the same as
// on built in types: the numbers in question must be of the same type
// (i.e. floats or integers).
fn main() {
    let complex_num1 = num::complex::Complex::new(10.0, 20.0); // Must use floats
    let complex_num2 = num::complex::Complex::new(3.1, -4.2);

    let sum = complex_num1 + complex_num2;

    println!("Sum: {}", sum);
}
```

Creates complex numbers of type `num::complex::Complex`. Both the real and imaginary part of the complex number must be of the same type.

## References

1. [Official docs](https://rust-lang-nursery.github.io/rust-cookbook/science/mathematics/complex_numbers.html)
2. [RustConf 2023 - Implementing a Blazingly Fast Quantum State Simulator in Rust](https://www.youtube.com/watch?v=5wkDXSP3mCc)
