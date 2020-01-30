extern crate serde;
extern crate serde_json;

// derive serialization
use serde::{Deserialize, Serialize};

// discord api specific
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::{channel::Attachment, prelude::*};
use serenity::prelude::*;

// filesystem bits
use std::fs::{create_dir_all, read_dir, File, OpenOptions};
use std::io::prelude::*;

// use utilities
use crate::utils::zip::zipit;

pub fn say(ctx: &mut Context, msg: &Message, text: &str) {
    match msg.channel_id.say(&ctx.http, text) {
        Ok(_) => (),
        Err(e) => println!("Could not send message to user! Error: {}", e),
    }
}

#[command]
#[description = "Tells you some info on why this system is here"]
pub fn explain(ctx: &mut Context, msg: &Message) -> CommandResult {
    let reply = "Hey!\n\nThis it the mYi receipt processing system. For more info ask Domi!";
    say(ctx, msg, reply);
    Ok(())
}

#[command]
#[description = "Submit receipts to your folder"]
pub fn add(ctx: &mut Context, msg: &Message) -> CommandResult {
    fn handle_attachment(
        msg: &Message,
        content: &[u8],
        attachment: &Attachment,
    ) -> Result<(), std::io::Error> {
        // get the year month value
        let date = msg.timestamp.date().format("%Y%m");

        // check if directory exists for this month and if not mkdir it
        create_dir_all(format!("files/{}/{}", msg.author.name, date))?;

        // write the file into a folder
        let mut handle = File::create(format!(
            "files/{}/{}/{}",
            msg.author.name, date, &attachment.filename
        ))?;
        handle.write_all(content)?;

        Ok(())
    };

    // simplifying the code
    let fs = &msg.attachments;

    // check if the attachments contain one and only one file and process it all
    match (fs.len(), fs.get(0)) {
        // handle when no attachments are provided
        (0, _) => say(
            ctx,
            msg,
            "You haven't provided any attachments. Aborting...",
        ),

        // handle when one attachment is provided. main brains of the function
        (1, Some(attachment)) => {
            if let Ok(content) = attachment.download() {
                match handle_attachment(&msg, &content, &attachment) {
                    Ok(_) => {
                        handle_log(msg, &attachment.filename);
                        println!("Written file {} sucessfully", &attachment.filename);
                        say(
                            ctx,
                            msg,
                            &*format!(
                                "Your file ({}) has been sucessfully saved.",
                                &attachment.filename
                            ),
                        )
                    }
                    Err(e) => println!(
                        "Cannot save file the file provided by the user. Error: {}",
                        e
                    ),
                }
            } else {
                say(
                    ctx,
                    msg,
                    &*format!(
                        "Could not download file ({}). Please try again.",
                        &attachment.filename
                    ),
                );
            }
        }

        // if more then 1 attachment then we handle that.
        (_, _) => say(ctx, msg, "Please only provide one attachment."),
    }
    Ok(())
}

#[command]
#[description = "Allows for retrieval of a given months receipt files.\n\n Use `!invoice get yyyymm`"]
fn get(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    use std::collections::hash_map::DefaultHasher;
    use std::fs::remove_file;
    use std::hash::{Hash, Hasher};

    // remove repetition
    let name = &msg.author.name;

    // get all the months available to the user
    let available_folders = match read_dir(format!("files/{}", msg.author.name)) {
        Ok(list) => {
            let temp = list
                .map(|d| d.unwrap().file_name().into_string().unwrap())
                .collect::<Vec<String>>();

            if temp.is_empty() {
                String::from("No files available for this user.")
            } else {
                temp.join("\n")
            }
        }
        Err(_) => String::from("No files available for this user."),
    };

    // handle argument passed in by the user
    let argument = match args.single::<u32>() {
        Ok(val) => val,
        Err(_) => {
            say(
                ctx,
                msg,
                "Sorry, could not find or parse date parameter.\nUse get as `!invoice get yyyymm`",
            );
            return Ok(());
        }
    };

    // start the process
    let path = format!("./files/{}/{}", name, argument);
    say(ctx, msg, "Please wait...");

    // get a file name here based on user id hash
    let mut s = DefaultHasher::new();
    format!("{}{}", name, argument).hash(&mut s);
    let zipfile = format!("{}.zip", s.finish());

    // zip it all
    match zipit(&path, &zipfile) {
        Ok(_) => match msg
            .channel_id
            .send_files(&ctx.http, vec![zipfile.as_str()], |m| {
                m.content("Your invoices.")
            }) {
            Ok(_) => (),
            Err(e) => println!("Could not send the zip file to the user. Error: {}", e),
        },
        Err(e) => {
            say(
                ctx,
                msg,
                &format!(
                    "Could not retrieve files.\nExisting data for user:\n\n{}",
                    available_folders
                ),
            );
            println!("Could not zip files: Error: {}", e);
        }
    };

    // clean up the zip file
    match remove_file(&zipfile) {
        Ok(()) => println!("Cleaned up {}", zipfile),
        Err(e) => println!("Could not clean up file {}. Error: {}", zipfile, e),
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct LogEntry {
    date: String,
    author: String,
    file: String,
}

fn handle_log(msg: &Message, filename: &str) {
    let file = OpenOptions::new().create(true).append(true).open("log.txt");
    let entry = LogEntry {
        date: format!("{}", &msg.id.created_at()),
        author: (&*msg.author.name).to_string(),
        file: filename.to_owned(),
    };
    file.unwrap().write_all(
        serde_json::to_string(&entry)
            .expect("Could not serialise log entry")
            .as_bytes(),
    );
}
