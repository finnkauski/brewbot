// serenity
use serenity::client::Client;
use serenity::framework::standard::{
    help_commands,
    macros::{check, command, group, help},
    Args, CheckResult, CommandGroup, CommandOptions, CommandResult, DispatchError, HelpOptions,
    StandardFramework,
};
use serenity::http::Http;
use serenity::model::{
    channel::Message,
    gateway::Ready,
    id::{ChannelId, MessageId, UserId},
};
use serenity::prelude::{Context, EventHandler, RwLock, TypeMapKey};

// event dispatcher and scheduler for tasks
// We will use this crate as event dispatcher.
use hey_listen::sync::{
    ParallelDispatcher as Dispatcher, ParallelDispatcherRequest as DispatcherRequest,
};
// And this crate to schedule our tasks.
use white_rabbit::{DateResult, Duration, Scheduler, Utc};

// utils
use dotenv::dotenv;
use std::{
    collections::HashSet,
    env,
    hash::{Hash, Hasher},
    sync::Arc,
};

#[group]
#[commands(hal, hi, rme)]
struct General;

// Event handler
struct Handler;

impl EventHandler for Handler {
    // fn guild_member_ddition
    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

// Event dispatch stuff
#[derive(Clone)]
enum DispatchEvent {
    ReactEvent(MessageId, UserId),
}

// We need to implement equality for our enum.
// One could test variants only. In this case, we want to know who reacted
// on which message.
impl PartialEq for DispatchEvent {
    fn eq(&self, other: &DispatchEvent) -> bool {
        match (self, other) {
            (
                DispatchEvent::ReactEvent(self_message_id, self_user_id),
                DispatchEvent::ReactEvent(other_message_id, other_user_id),
            ) => self_message_id == other_message_id && self_user_id == other_user_id,
        }
    }
}

impl Eq for DispatchEvent {}

// See following Clippy-lint:
// https://rust-lang.github.io/rust-clippy/master/index.html#derive_hash_xor_eq
impl Hash for DispatchEvent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            DispatchEvent::ReactEvent(msg_id, user_id) => {
                msg_id.hash(state);
                user_id.hash(state);
            }
        }
    }
}

struct DispatcherKey;
impl TypeMapKey for DispatcherKey {
    type Value = Arc<RwLock<Dispatcher<DispatchEvent>>>;
}

struct SchedulerKey;
impl TypeMapKey for SchedulerKey {
    type Value = Arc<RwLock<Scheduler>>;
}

// Commands
#[command]
#[description = "Says hi!"]
#[bucket = "nospam"]
fn hi(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "こんにちは!")?;
    println!(
        "id: {} - user: {}: said hi!",
        msg.author.name, msg.author.id
    );
    Ok(())
}

#[command]
#[description = "HAL-9000"]
#[checks(Owner)]
fn hal(ctx: &mut Context, msg: &Message) -> CommandResult {
    let contents = ":red_circle: Yes, Artie?";
    if let Err(why) = msg.channel_id.say(&ctx.http, &contents) {
        println!("Error sending message: {:?}", why);
    };
    Ok(())
}

// Reminder Command
// Just a helper-function for creating the closure we want to use as listener.
// It saves us from writing the same trigger twice for repeated and non-repeated
// tasks (see remind-me command below).
fn thanks_for_reacting(
    http: Arc<Http>,
    channel: ChannelId,
) -> Box<dyn Fn(&DispatchEvent) -> Option<DispatcherRequest> + Send + Sync> {
    Box::new(move |_| {
        if let Err(why) = channel.say(&http, "Thanks for reacting!") {
            println!("Could not send message: {:?}", why);
        }

        Some(DispatcherRequest::StopListening)
    })
}

#[command]
#[aliases("rm")]
fn rme(context: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // It might be smart to set a moderately high minimum value for `time`
    // to avoid abuse like tasks that repeat every 100ms, especially since
    // channels have send-message rate limits.
    let time: u64 = args.single()?;
    let repeat: bool = args.single()?;
    let args = args.rest().to_string();

    let scheduler = {
        let mut context = context.data.write();
        context
            .get_mut::<SchedulerKey>()
            .expect("Expected Scheduler.")
            .clone()
    };

    let dispatcher = {
        let mut context = context.data.write();
        context
            .get_mut::<DispatcherKey>()
            .expect("Expected Dispatcher.")
            .clone()
    };

    let http = context.http.clone();
    let msg = msg.clone();

    let mut scheduler = scheduler.write();

    // First, we check if the user wants a repeated task or not.
    if repeat {
        // Chrono's duration can also be negative
        // and therefore we cast to `i64`.
        scheduler.add_task_duration(Duration::milliseconds(time as i64), move |_| {
            let bot_msg = match msg.channel_id.say(&http, &args) {
                Ok(msg) => msg,
                // We could not send the message, thus we will try sending it
                // again in five seconds.
                // It might be wise to keep a counter for maximum tries.
                // If the channel got deleted, trying to send a message will
                // always fail.
                Err(why) => {
                    println!("Error sending message: {:?}.", why);

                    return DateResult::Repeat(Utc::now() + Duration::milliseconds(5000));
                }
            };

            let http = http.clone();

            // We add a function to dispatch for a certain event.
            dispatcher.write().add_fn(
                DispatchEvent::ReactEvent(bot_msg.id, msg.author.id),
                // The `thanks_for_reacting`-function creates a function
                // to schedule.
                thanks_for_reacting(http, bot_msg.channel_id),
            );

            // We return that our date shall happen again, therefore we need
            // to tell when this shall be.
            DateResult::Repeat(Utc::now() + Duration::milliseconds(time as i64))
        });
    } else {
        // Pretty much identical with the `true`-case except for the returned
        // variant.
        scheduler.add_task_duration(Duration::milliseconds(time as i64), move |_| {
            let bot_msg = match msg.channel_id.say(&http, &args) {
                Ok(msg) => msg,
                Err(why) => {
                    println!("Error sending message: {:?}.", why);

                    return DateResult::Repeat(Utc::now() + Duration::milliseconds(5000));
                }
            };
            let http = http.clone();

            dispatcher.write().add_fn(
                DispatchEvent::ReactEvent(bot_msg.id, msg.author.id),
                thanks_for_reacting(http, bot_msg.channel_id),
            );

            // The task is done and that's it, we do not to repeat it.
            DateResult::Done
        });
    };

    Ok(())
}

// Runners
fn main() {
    dotenv().ok();
    // Login with a bot token from the environment
    let mut client = Client::new(&env::var("DISCORD_TOKEN").expect("token"), Handler)
        .expect("Error creating client");

    // Set up stuff for dispatching events on a scheduled basis
    {
        let mut data = client.data.write();
        let scheduler = Scheduler::new(4);
        let scheduler = Arc::new(RwLock::new(scheduler));
        let mut dispatcher: Dispatcher<DispatchEvent> = Dispatcher::default();
        dispatcher
            .num_threads(4)
            .expect("Could not construct dispatcher threadpool");
        data.insert::<DispatcherKey>(Arc::new(RwLock::new(dispatcher)));
        data.insert::<SchedulerKey>(scheduler);
    }

    client.with_framework(
        StandardFramework::new()
            .configure(|c| c.prefix("!"))
            .on_dispatch_error(|ctx, msg, error| {
                if let DispatchError::Ratelimited(seconds) = error {
                    let _ = msg.channel_id.say(
                        &ctx.http,
                        &format!("Try this again in {} seconds.", seconds),
                    );
                }
            })
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

// Help
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

// Permissions checks
#[check]
#[name = "Owner"]
fn owner_check(_: &mut Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> CheckResult {
    (msg.author.id == 104_970_046_811_435_008).into()
}
