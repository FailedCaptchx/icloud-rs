use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //let loc = dirs::config_dir().unwrap().join("nefos-lib").join("session.json");
    let cookie_file = dirs::config_dir().unwrap().join("nefos-lib").join("cookies.json");
    let service;
    if let Ok(o) = try_to_read_save() {
        service = nefos_lib::Session::import_from_string(o, "cookies.json").await?;
    } else {
        service = {
            let apple_id = std::env::var("APPLE_ID").unwrap();
            let password = std::env::var("PASSWORD").unwrap();
            let sesh = nefos_lib::Session::new(apple_id, password, "cookies.json", None).await?;
            use std::io::{stdin,stdout,Write};
            let mut s=String::new();
            print!("Code: ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            }
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            }
            sesh.hsa(s).await.unwrap()
        };
        {
            let data = service.serialize_session()?;
            println!("{data}");
            let mut f = BufWriter::new(File::create("session.json")?);
            f.write_all(&data.as_bytes())?;
            f.flush()?;
        }
    }
    service.save_cookies();
    println!("{}", service.get_name().await);
    //service.auth("daniel@weilxnd.com".to_string(), "Ryansnet2006!@#".to_string()).await;
    println!("{}", service.fetch_calendars("America/Chicago", "2023-11-01", "2023-11-30").await?);
    Ok(())
}

fn try_to_read_save() -> std::io::Result<String> {
    let mut buf: String = String::new();
    let mut f = BufReader::new(File::open("session.json")?);
    f.read_to_string(&mut buf)?;
    Ok(buf)
}