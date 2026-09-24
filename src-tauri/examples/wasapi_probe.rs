//! WASAPI 独占模式探测：用 wasapi crate 官方示例的模式实测本机默认设备
use wasapi::*;

fn main() {
    initialize_mta().unwrap();

    println!("== 渲染设备列表 ==");
    let collection = DeviceCollection::new(&Direction::Render).unwrap();
    for d in &collection {
        println!("  - {}", d.expect("dev").get_friendlyname().unwrap_or_default());
    }

    let device = get_default_device(&Direction::Render).unwrap();
    let dev_name = device.get_friendlyname().unwrap_or_default();
    println!("\n== 默认设备：{dev_name} ==");

    let mut audio_client = device.get_iaudioclient().unwrap();
    let mix = audio_client.get_mixformat().unwrap();
    println!(
        "  混合格式：{}Hz × {}ch，{} 位存储 / {} 位有效（{:?}）",
        mix.get_samplespersec(),
        mix.get_nchannels(),
        mix.get_bitspersample(),
        mix.get_validbitspersample(),
        mix.get_subformat().unwrap_or(SampleType::Int)
    );
    let (def_period, min_period) = audio_client.get_periods().unwrap();
    println!(
        "  周期：默认 {}（{:.1}ms），最小 {}（{:.1}ms）",
        def_period,
        def_period as f64 / 10_000.0,
        min_period,
        min_period as f64 / 10_000.0
    );

    let ch = mix.get_nchannels() as usize;
    let mut combos: Vec<(usize, usize, SampleType, u32)> = Vec::new();
    for &(store, valid, kind) in &[
        (32usize, 24usize, SampleType::Int),
        (32, 32, SampleType::Int),
        (24, 24, SampleType::Int),
        (16, 16, SampleType::Int),
        (32, 32, SampleType::Float),
    ] {
        for &rate in &[44100u32, 48000] {
            combos.push((store, valid, kind, rate));
        }
    }

    println!("\n== 独占初始化实测（周期=设备默认）==");
    for (store, valid, kind, rate) in combos {
        let wf = WaveFormat::new(store, valid, &kind, rate as usize, ch, None);
        let isfs = match audio_client.is_supported_exclusive_with_quirks(&wf) {
            Ok(_) => "quirks支持".to_string(),
            Err(e) => {
                let c = e
                    .downcast_ref::<windows::core::Error>()
                    .map(|w| format!("0x{:08X}", w.code().0 as u32))
                    .unwrap_or_default();
                format!("quirks拒绝({c})")
            }
        };
        let mut client = device.get_iaudioclient().unwrap();
        let result = client.initialize_client(
            &wf,
            def_period,
            &Direction::Render,
            &ShareMode::Exclusive,
            false,
        );
        let code = result.as_ref().err().and_then(|e| {
            e.downcast_ref::<windows::core::Error>()
                .map(|w| format!("0x{:08X}", w.code().0 as u32))
        });
        match result {
            Ok(_) => println!(
                "  [成功] {rate}Hz {store}/{valid} {kind:?} → 初始化 OK（{isfs}）"
            ),
            Err(_) => println!(
                "  [失败] {rate}Hz {store}/{valid} {kind:?} → {code:?}（{isfs}）"
            ),
        }
        drop(client);
    }
}
