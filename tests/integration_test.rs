use rusp::compiler::compile;
use rusp::vm::VM;

#[test]
fn actually_e2e() {
    let src = r#"
(defun (a b) ((if b * +) 2 3))

(defun (_add d e) (+ d e))

(print (* (a true) ; 6
          (_add 2 3))) ; 5
; 30
"#
    .to_owned();

    let bc = compile(&src);
    let mut vm = VM::default();
    vm.run(bc);
}

// #[test]
// fn actually_e2e_2() {
//     let src = r#"
// (defun (fib n)
//     (if (< n 2)
//         n
//         (+ (fib (- n 1))
//            (fib (- n 2)))))

// (print (fib 20))
// "#
//     .to_owned();
//     let bc = compile(&src);
//     let fib = |n: i64| -> i64 {
//         let mut a = 0;
//         let mut b = 1;
//         for _ in 0..n {
//             let c = a + b;
//             a = b;
//             b = c;
//         }
//         a
//     };

//     let mut vm = VM::default();
//     vm.run(bc);
//     // assert_eq!(fib(20), *vm.stack.at(0).unwrap().as_integer().unwrap());
// }

fn run_code(src: &str) {
    let bc = compile(&src.to_string());
    // println!("{}", disassemble(&bc));
    VM::default().run(bc);
}

#[test]
fn target_spec_1() {
    run_code("(print \"asdf\")");
}

#[test]
fn test_cons() {
    run_code("(print (cons 1 2)");
}

#[test]
fn target_spec_2() {
    run_code("(print 'asdf)");
}

#[test]
fn target_spec_3() {
    run_code("(print '(asdf))");
}

#[test]
fn target_spec_4() {
    run_code("(print '(asdf 1))");
}

#[test]
fn target_spec_5() {
    run_code("(print '(asdf (1)))");
}

#[test]
fn target_spec_6() {
    run_code("(print '(asdf '(1)))");
}

#[test]
fn target_spec_7() {
    run_code("(print ''1)");
}

#[test]
fn target_spec_8() {
    run_code("(print '\"a\")");
}

#[test]
fn target_spec_9() {
    run_code("(print (quote a))");
}

#[test]
fn target_spec_10() {
    run_code(
        r#"
(defun (f)
    (define red-herring 10)
    (defun (g b)
        (+ b 1))
    g)
(print ((f) 8))
"#,
    );
}

#[test]
fn target_spec_101() {
    run_code(
        r#"
(defun (f)
    (defun (g) "asdf")
    g)
(print (f)) ; g
(print ((f))) ; "asdf"
"#,
    );
}

#[test]
fn closures_lifted() {
    run_code(
        r#"
(defun (f a)
    (define b 10)
    (define c 11)
    (defun (g)
        (print a)
        (print b)
        (print c))
    g)

(define closure (f "a"))
(closure)
(closure)
"#,
    );
}

#[test]
fn closures_lifted_mut() {
    run_code(
        r#"
(defun (make-counter)
    (define x 0)
    (defun (count)
        (print x)
        (set x (inc x)))
    count)

(define counter (make-counter))
(counter)
(counter)
(counter)
"#,
    );
}

#[test]
fn sibling_closures_lifted_mut() {
    run_code(
        r#"
(defun (make-counters)
    (define x 0)
    (defun (count1)
        (print x)
        (set x (inc x)))

    (defun (count2)
        (print x)
        (set x (inc x)))

    (defun (both)
        (count1)
        (count2))
    '(count1 count2 both))

(define all (make-counters))
(print all)

; (print (car all))
; (print (car (cdr all)))
; (print (car (cdr (cdr all))))
; (both)
; (both)
; (both)
"#,
    );
}

#[test]
fn target_spec_11() {
    run_code(
        r#"
(defun (f)
    (defun (g)
        (defun (h) "asdf")
        h)
    g)
(print (f)) ; g
(print ((f))) ; h
(print (((f)))) ; "asdf"
"#,
    );
}

#[test]
fn target_spec_12() {
    run_code(
        r#"
(defun (foo arg1)
    (define local "baz")
    (print local)
    (print arg1))

(foo "bar")
"#,
    );
}

#[test]
fn target_spec_13() {
    run_code(
        r#"
(defun (f)
    (define bar "baz")
    (defun (g)
        (print bar))
    (g))

(f)
"#,
    );
}

#[test]
fn target_spec_14() {
    run_code(
        r#"
(defun (f)
    (define bar "baz")
    (defun (g)
        (defun (h)
            (print bar))
        h)
    ((g))
)

(f)
"#,
    );
}

#[test]
fn target_spec_15() {
    run_code(
        r#"
(defun (f)
    (define bar "baz")
    (defun (g)
        (print bar))
    (g))

(f)
"#,
    );
}

#[test]
fn playground() {
    run_code(
        r#"
(defun (f)
    "returned from f")
(f)

(defun (g a)
    "returned from g")
(g "asdf")

(defun (h)
    (define x 10)
    "returned from h")
(h)

(defun (i a)
    (define x 10)
    "returned from i")
(i "asdf")
"#,
    )
}

// #[test]
// fn target_spec() {
//     let src = r#"
// (fn (fib n)
//     (if (< n 2)
//         n
//         (+ (fib (- n 1))
//            (fib (- n 2)))))

// (print (fib 20))

// (define foo "bar")

// (print foo)

// ; inner functions
// (defun (fib-iter n)
//     (defun (inner a b n)
//         (if (= n 0)
//             a
//             (inner b (+ a b) (- n 1))))
//     (inner 0 1 n))

// (print (fib-iter 20))
// (print "^ should be 6765")

// ; stateful functions + returning allocated values
// (defun (stateful)
//     (define x 0)
//     (print (concat "returning " (stringify x)))
//     (+ x 4))
// (define y (stateful))
// (print "^ should print 'returning 0'")
// (print y)
// (print "^ should be 4")

// ; closures
// (defun (make-adder x)
//     (fn (y) (+ x y)))
// (define add10 (make-adder 10))
// (print (add10 5))
// (print "^ should be 15")

// ; stateful closures
// (defun (counter)
//     (define x 0)
//     (fn ()
//         (set! x (+ x 1))
//         x)
// (define c (counter))
// (print (c))
// (print "^ should be 1"
// (print (c))
// (print "^ should be 2"
// (print (c))
// (print "^ should be 3"

// ; higher order functions
// (defun (apply-twice f x)
//     (f (f x)))

// (print (apply-twice (make-adder 10) 5))
// (print "^ should be 25")

// ; cons cells
// (define l '(1 2 3 4 5))
// (print "^ should be (1 2 3 4 5)"

// (print (car l))
// (print "^ should be 1")

// (print (cdr l))
// (print "^ should be (2 3 4 5)"
// "#
//     .to_owned();

//     let bc = compile(&src);

//     let fib = |n: i64| -> i64 {
//         let mut a = 0;
//         let mut b = 1;
//         for _ in 0..n {
//             let c = a + b;
//             a = b;
//             b = c;
//         }
//         a
//     };

//     let mut vm = VM::default();
//     vm.run(bc);
//     assert_eq!(fib(20), *vm.stack.at(0).unwrap().as_integer().unwrap());
// }
