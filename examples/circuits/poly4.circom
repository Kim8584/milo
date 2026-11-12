pragma circom 2.0.0;

template Poly4() {
    signal input x;
    signal input p1;
    signal input p2;
    signal input p3;
    signal input p4;
    signal output out;

    signal x2;
    signal x3;
    signal x4;

    x2 <== x * x;
    x3 <== x2 * x;
    x4 <== x3 * x;

    signal t1;
    signal t2;
    signal t3;
    signal t4;

    t1 <== p1 * x;
    t2 <== p2 * x2;
    t3 <== p3 * x3;
    t4 <== p4 * x4;

    out <== t1 + t2 + t3 + t4;
}

component main {public [p1, p2, p3, p4]} = Poly4();
