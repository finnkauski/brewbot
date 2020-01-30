// serenity
use serenity::client::Client;
use serenity::framework::standard::{
    help_commands,
    macros::{check, command, group, help},
    Args, CheckResult, CommandGroup, CommandOptions, CommandResult, HelpOptions, StandardFramework,
};
use serenity::model::channel::Message;
use serenity::model::id::UserId;
use serenity::prelude::{Context, EventHandler};
use std::collections::HashSet;

// utils
use dotenv::dotenv;
use std::env;

#[group]
#[commands(hal, hi)]
struct General;

// #[group]
// #[prefix = "git"]
// #[commands(hey)]
// struct Git;

struct Handler;

impl EventHandler for Handler {}

fn main() {
    dotenv().ok();
    // Login with a bot token from the environment
    let mut client = Client::new(&env::var("DISCORD_TOKEN").expect("token"), Handler)
        .expect("Error creating client");
    client.with_framework(
        StandardFramework::new()
            .configure(|c| c.prefix("!")) // set the bot's prefix to "~"
            .unrecognised_command(|_, _, unknown_command_name| {
                println!("Could not find command: '{}'", unknown_command_name);
            })
            .group(&GENERAL_GROUP)
            .bucket("nospam", |b| b.delay(5))
            .help(&MY_HELP),
    );

    // start listening for events by starting a single shard
    if let Err(why) = client.start() {
        println!("An error occurred while running the client: {:?}", why);
    }
}

// help setup
#[help]
fn my_help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}

// Commands
//
#[command]
#[description = "Says hi!"]
#[bucket = "nospam"]
fn hi(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "こんにちは!")?;
    println!("id: {} - user: {}: said hi", msg.author.name, msg.author.id);
    Ok(())
}

#[command]
#[description = "HAL-9000"]
#[checks(Owner)]
fn hal(ctx: &mut Context, msg: &Message) -> CommandResult {
    let contents = format!(":red_circle: Yes, {}?", msg.author.name);
    if let Err(why) = msg.channel_id.say(&ctx.http, &contents) {
        println!("Error sending message: {:?}", why);
    };
    Ok(())
}

#[check]
#[name = "Owner"]
fn owner_check(_: &mut Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> CheckResult {
    (msg.author.id == 104_970_046_811_435_008).into()
}
