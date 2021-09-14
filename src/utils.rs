use crate::*;

struct SizeUnit {
    partial: f64,
    partial_unit: char,
    total: f64,
    total_unit: char,
}

pub fn print_updates(received: f64, header: &TeleportInit) {
    let percent: f64 = (received as f64 / header.filesize as f64) * 100f64;
    let units = convert_units(received as f64, header.filesize as f64);
    print!(
        "\r => {:>8.03}{} of {:>8.03}{} ({:02.02}%)",
        units.partial, units.partial_unit, units.total, units.total_unit, percent
    );
    io::stdout().flush().unwrap();
}

fn convert_units(mut partial: f64, mut total: f64) -> SizeUnit {
    let unit = ['B', 'K', 'M', 'G', 'T'];
    let mut out = SizeUnit {
        partial: 0.0,
        partial_unit: 'B',
        total: 0.0,
        total_unit: 'B',
    };

    let mut count = 0;
    loop {
        if (total / 1024.0) > 1.0 {
            count += 1;
            total /= 1024.0;
        } else {
            break;
        }
        if count == unit.len() - 1 {
            break;
        }
    }
    out.total = total;
    out.total_unit = unit[count];

    count = 0;
    loop {
        if (partial / 1024.0) > 1.0 {
            count += 1;
            partial /= 1024.0;
        } else {
            break;
        }
        if count == unit.len() - 1 {
            break;
        }
    }
    out.partial = partial;
    out.partial_unit = unit[count];
    out
}
