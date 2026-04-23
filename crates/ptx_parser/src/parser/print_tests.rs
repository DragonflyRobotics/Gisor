
//! Run tests with:   
//! cargo test parser::print_tests -- --nocapture


use crate::parser::parse;



fn show(label: &str, ptx: &str) {
    println!("\n========================================");
    println!("  {label}");
    println!("========================================");
    match parse(ptx) {
        Ok(kernel) => {
            println!("kernel name: {}", kernel.name);
            println!("params:");
            for (i, p) in kernel.params.iter().enumerate() {
                println!("  [{i}] {} : {:?}", p.name, p.ptx_type);
            }

            println!("instructions ({} total):", kernel.instructions.len());
            for (pc, inst) in kernel.instructions.iter().enumerate() {
                println!("  [{pc:2}] {:?} args={:?}", inst.inst_type, inst.args);
            }
        }
        Err(e) => {
            println!("PARSE ERROR: {e}");
        }
    }
}

#[test]
fn snippet_01_single_mad() {
    show(
        "snippet 1: single mad.lo.s32",
        r#"
        .visible .entry foo()
        {
            mad.lo.s32 %r1, %r5, %r4, %r3;
            ret;
        }
        "#,
    );
}

#[test]
fn snippet_02_ld_param_and_store() {
    show(
        "snippet 2: ld.param and st.global",
        r#"
        .visible .entry foo(
            .param .u64 foo_param_0,
            .param .u32 foo_param_1
        )
        {
            ld.param.u64 %rd1, [foo_param_0];
            ld.param.u32 %r2, [foo_param_1];
            st.global.f32 [%rd1], %f3;
            ret;
        }
        "#,
    );
}

#[test]
fn snippet_03_special_registers() {
    show(
        "snippet 3: thread/block ID reads",
        r#"
        .visible .entry foo()
        {
            mov.u32 %r3, %tid.x;
            mov.u32 %r4, %ntid.x;
            mov.u32 %r5, %ctaid.x;
            mov.u32 %r6, %tid.y;
            ret;
        }
        "#,
    );
}

#[test]
fn snippet_04_branch_with_label() {
    show(
        "snippet 4: predicated branch resolves to PC",
        r#"
        .visible .entry foo()
        {
            setp.ge.s32 %p1, %r1, %r2;
            @%p1 bra $L_skip;
            add.s64 %rd1, %rd2, %rd3;
            add.s64 %rd4, %rd5, %rd6;
        $L_skip:
            ret;
        }
        "#,
    );
}

#[test]
fn snippet_05_immediate_vs_register() {
    show(
        "snippet 5: add.s32 with register vs immediate",
        r#"
        .visible .entry foo()
        {
            add.s32 %r1, %r2, %r3;
            add.s32 %r4, %r5, 10;
            mul.wide.s32 %rd1, %r1, 4;
            ret;
        }
        "#,
    );
}

#[test]
fn snippet_06_full_add_kernel() {
    show(
        "snippet 6: full addKernel from the project notes",
        r#"
.version 9.1
.target sm_75
.address_size 64

.visible .entry _Z9addKernelPfS_S_i(
    .param .u64 _Z9addKernelPfS_S_i_param_0,
    .param .u64 _Z9addKernelPfS_S_i_param_1,
    .param .u64 _Z9addKernelPfS_S_i_param_2,
    .param .u32 _Z9addKernelPfS_S_i_param_3
)
{
    .reg .pred     %p<2>;
    .reg .f32     %f<4>;
    .reg .b32     %r<6>;
    .reg .b64     %rd<11>;

    ld.param.u64     %rd1, [_Z9addKernelPfS_S_i_param_0];
    ld.param.u64     %rd2, [_Z9addKernelPfS_S_i_param_1];
    ld.param.u64     %rd3, [_Z9addKernelPfS_S_i_param_2];
    ld.param.u32     %r2, [_Z9addKernelPfS_S_i_param_3];
    mov.u32     %r3, %tid.x;
    mov.u32     %r4, %ntid.x;
    mov.u32     %r5, %ctaid.x;
    mad.lo.s32     %r1, %r5, %r4, %r3;
    setp.ge.s32     %p1, %r1, %r2;
    @%p1 bra     $L__BB0_2;

    cvta.to.global.u64     %rd4, %rd1;
    mul.wide.s32     %rd5, %r1, 4;
    add.s64     %rd6, %rd4, %rd5;
    cvta.to.global.u64     %rd7, %rd2;
    add.s64     %rd8, %rd7, %rd5;
    ld.global.f32     %f1, [%rd8];
    ld.global.f32     %f2, [%rd6];
    add.f32     %f3, %f2, %f1;
    cvta.to.global.u64     %rd9, %rd3;
    add.s64     %rd10, %rd9, %rd5;
    st.global.f32     [%rd10], %f3;

$L__BB0_2:
    ret;
}
"#,
    );
}