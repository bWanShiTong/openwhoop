use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};
use strum::EnumIter;

#[derive(Clone, Copy, Debug)]
pub struct ActivityPeriod {
    pub period_id: NaiveDate,
    pub from: NaiveDateTime,
    pub to: Option<NaiveDateTime>,
    pub activity: ActivityType,
    pub strain: Option<f64>,
}

#[derive(Deserialize, Debug)]
pub enum Category {
    #[serde(rename = "CARDIOVASCULAR")]
    CardioVascular,
    #[serde(rename = "NON_CARDIO")]
    NonCardio,
    #[serde(rename = "MUSCULAR")]
    Muscular,
    #[serde(rename = "RESTORATIVE")]
    Restorative,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, EnumIter)]
pub enum ActivityType {
    #[serde(rename = "Activity")]
    Activity = -1,
    #[serde(rename = "Running")]
    Running = 0,
    #[serde(rename = "Cycling")]
    Cycling = 1,
    #[serde(rename = "Baseball")]
    Baseball = 16,
    #[serde(rename = "Basketball")]
    Basketball = 17,
    #[serde(rename = "Rowing")]
    Rowing = 18,
    #[serde(rename = "Fencing")]
    Fencing = 19,
    #[serde(rename = "Field Hockey")]
    FieldHockey = 20,
    #[serde(rename = "Football")]
    Football = 21,
    #[serde(rename = "Golf")]
    Golf = 22,
    #[serde(rename = "Ice Hockey")]
    IceHockey = 24,
    #[serde(rename = "Lacrosse")]
    Lacrosse = 25,
    #[serde(rename = "Rugby")]
    Rugby = 27,
    #[serde(rename = "Sailing")]
    Sailing = 28,
    #[serde(rename = "Skiing")]
    Skiing = 29,
    #[serde(rename = "Soccer")]
    Soccer = 30,
    #[serde(rename = "Softball")]
    Softball = 31,
    #[serde(rename = "Squash")]
    Squash = 32,
    #[serde(rename = "Swimming")]
    Swimming = 33,
    #[serde(rename = "Tennis")]
    Tennis = 34,
    #[serde(rename = "Track & Field")]
    TrackField = 35,
    #[serde(rename = "Volleyball")]
    Volleyball = 36,
    #[serde(rename = "Water Polo")]
    WaterPolo = 37,
    #[serde(rename = "Wrestling")]
    Wrestling = 38,
    #[serde(rename = "Boxing")]
    Boxing = 39,
    #[serde(rename = "Dance")]
    Dance = 42,
    #[serde(rename = "Pilates")]
    Pilates = 43,
    #[serde(rename = "Yoga")]
    Yoga = 44,
    #[serde(rename = "Weightlifting")]
    Weightlifting = 45,
    #[serde(rename = "Canoeing")]
    Canoeing = 46,
    #[serde(rename = "Cross Country Skiing")]
    CrossCountrySkiing = 47,
    #[serde(rename = "Functional Fitness")]
    FunctionalFitness = 48,
    #[serde(rename = "Duathlon")]
    Duathlon = 49,
    #[serde(rename = "Machine Workout")]
    MachineWorkout = 50,
    #[serde(rename = "Gymnastics")]
    Gymnastics = 51,
    #[serde(rename = "Hiking/Rucking")]
    HikingRucking = 52,
    #[serde(rename = "Horseback Riding")]
    HorsebackRiding = 53,
    #[serde(rename = "Jogging")]
    Jogging = 54,
    #[serde(rename = "Kayaking")]
    Kayaking = 55,
    #[serde(rename = "Martial Arts")]
    MartialArts = 56,
    #[serde(rename = "Mountain Biking")]
    MountainBiking = 57,
    #[serde(rename = "Obstacle Racing")]
    ObstacleRacing = 58,
    #[serde(rename = "Powerlifting")]
    Powerlifting = 59,
    #[serde(rename = "Rock Climbing")]
    RockClimbing = 60,
    #[serde(rename = "Paddleboarding")]
    Paddleboarding = 61,
    #[serde(rename = "Triathlon")]
    Triathlon = 62,
    #[serde(rename = "Walking")]
    Walking = 63,
    #[serde(rename = "Surfing")]
    Surfing = 64,
    #[serde(rename = "Elliptical")]
    Elliptical = 65,
    #[serde(rename = "Stairmaster")]
    Stairmaster = 66,
    #[serde(rename = "Plyometrics")]
    Plyometrics = 67,
    #[serde(rename = "Spinning")]
    Spinning = 68,
    #[serde(rename = "Sex")]
    Sex = 69,
    #[serde(rename = "Meditation")]
    Meditation = 70,
    #[serde(rename = "Other")]
    Other = 71,
    #[serde(rename = "Pit Practice")]
    PitPractice = 72,
    #[serde(rename = "Diving")]
    Diving = 73,
    #[serde(rename = "Operations - Tactical")]
    OperationsTactical = 74,
    #[serde(rename = "Operations - Medical")]
    OperationsMedical = 75,
    #[serde(rename = "Operations - Flying")]
    OperationsFlying = 76,
    #[serde(rename = "Operations - Water")]
    OperationsWater = 77,
    #[serde(rename = "Ultimate")]
    Ultimate = 82,
    #[serde(rename = "Climber")]
    Climber = 83,
    #[serde(rename = "Jumping Rope")]
    JumpingRope = 84,
    #[serde(rename = "Australian Rules Football")]
    AustralianRulesFootball = 85,
    #[serde(rename = "Skateboarding")]
    Skateboarding = 86,
    #[serde(rename = "Coaching")]
    Coaching = 87,
    #[serde(rename = "Ice Bath")]
    IceBath = 88,
    #[serde(rename = "Commuting")]
    Commuting = 89,
    #[serde(rename = "Gaming")]
    Gaming = 90,
    #[serde(rename = "Snowboarding")]
    Snowboarding = 91,
    #[serde(rename = "Motocross")]
    Motocross = 92,
    #[serde(rename = "Caddying")]
    Caddying = 93,
    #[serde(rename = "Obstacle Course Racing")]
    ObstacleCourseRacing = 94,
    #[serde(rename = "Motor Racing")]
    MotorRacing = 95,
    #[serde(rename = "HIIT")]
    Hiit = 96,
    #[serde(rename = "Spin")]
    Spin = 97,
    #[serde(rename = "Jiu Jitsu")]
    JiuJitsu = 98,
    #[serde(rename = "Manual Labor")]
    ManualLabor = 99,
    #[serde(rename = "Cricket")]
    Cricket = 100,
    #[serde(rename = "Pickleball")]
    Pickleball = 101,
    #[serde(rename = "Inline Skating")]
    InlineSkating = 102,
    #[serde(rename = "Box Fitness")]
    BoxFitness = 103,
    #[serde(rename = "Spikeball")]
    Spikeball = 104,
    #[serde(rename = "Wheelchair Pushing")]
    WheelchairPushing = 105,
    #[serde(rename = "Paddle Tennis")]
    PaddleTennis = 106,
    #[serde(rename = "Barre")]
    Barre = 107,
    #[serde(rename = "Stage Performance")]
    StagePerformance = 108,
    #[serde(rename = "High Stress Work")]
    HighStressWork = 109,
    #[serde(rename = "Parkour")]
    Parkour = 110,
    #[serde(rename = "Gaelic Football")]
    GaelicFootball = 111,
    #[serde(rename = "Hurling/Camogie")]
    HurlingCamogie = 112,
    #[serde(rename = "Circus Arts")]
    CircusArts = 113,
    #[serde(rename = "Resonance Frequency Breathing")]
    ResonanceFrequencyBreathing = 116,
    #[serde(rename = "Massage Therapy")]
    MassageTherapy = 121,
    #[serde(rename = "Strength Trainer")]
    StrengthTrainer = 123,
    #[serde(rename = "Watching Sports")]
    WatchingSports = 125,
    #[serde(rename = "Assault Bike")]
    AssaultBike = 126,
    #[serde(rename = "Kickboxing")]
    Kickboxing = 127,
    #[serde(rename = "Stretching")]
    Stretching = 128,
    #[serde(rename = "Other - Recovery")]
    OtherRecovery = 131,
    #[serde(rename = "Table Tennis/Ping Pong")]
    TableTennisPingPong = 230,
    #[serde(rename = "Badminton")]
    Badminton = 231,
    #[serde(rename = "Netball")]
    Netball = 232,
    #[serde(rename = "Sauna")]
    Sauna = 233,
    #[serde(rename = "Disc Golf")]
    DiscGolf = 234,
    #[serde(rename = "Yard Work/Gardening")]
    YardWorkGardening = 235,
    #[serde(rename = "Air Compression")]
    AirCompression = 236,
    #[serde(rename = "Percussive Massage")]
    PercussiveMassage = 237,
    #[serde(rename = "Paintball")]
    Paintball = 238,
    #[serde(rename = "Ice Skating")]
    IceSkating = 239,
    #[serde(rename = "Handball")]
    Handball = 240,
    #[serde(rename = "Percussive Massage (Hypervolt)")]
    PercussiveMassageHypervolt = 241,
    #[serde(rename = "Air Compression (Normatec)")]
    AirCompressionNormatec = 242,
    #[serde(rename = "Increase Relaxation")]
    IncreaseRelaxation = 243,
    #[serde(rename = "Increase Alertness")]
    IncreaseAlertness = 244,
    #[serde(rename = "Breathwork")]
    Breathwork = 245,
    #[serde(rename = "Non-Sleep Deep Rest")]
    NonSleepDeepRest = 246,
    #[serde(rename = "Steam Room")]
    SteamRoom = 247,
    #[serde(rename = "F45 Training")]
    F45Training = 248,
    #[serde(rename = "Padel")]
    Padel = 249,
    #[serde(rename = "Barry's")]
    BarryS = 250,
    #[serde(rename = "Dedicated Parenting")]
    DedicatedParenting = 251,
    #[serde(rename = "Stroller Walking")]
    StrollerWalking = 252,
    #[serde(rename = "Stroller Jogging")]
    StrollerJogging = 253,
    #[serde(rename = "Toddlerwearing")]
    Toddlerwearing = 254,
    #[serde(rename = "Babywearing")]
    Babywearing = 255,
    #[serde(rename = "Playing with Child")]
    PlayingWithChild = 256,
    #[serde(rename = "Cuddling with Child")]
    CuddlingWithChild = 257,
    #[serde(rename = "Barre3")]
    Barre3 = 258,
    #[serde(rename = "Hot Yoga")]
    HotYoga = 259,
    #[serde(rename = "Stadium Steps")]
    StadiumSteps = 261,
    #[serde(rename = "Polo")]
    Polo = 262,
    #[serde(rename = "Musical Performance")]
    MusicalPerformance = 263,
    #[serde(rename = "Kite Boarding")]
    KiteBoarding = 264,
    #[serde(rename = "Restorative Yoga")]
    RestorativeYoga = 265,
    #[serde(rename = "Dog Walking")]
    DogWalking = 266,
    #[serde(rename = "Water Skiing")]
    WaterSkiing = 267,
    #[serde(rename = "Wakeboarding")]
    Wakeboarding = 268,
    #[serde(rename = "Cooking")]
    Cooking = 269,
    #[serde(rename = "Cleaning")]
    Cleaning = 270,
    #[serde(rename = "Warm Bath")]
    WarmBath = 271,
    #[serde(rename = "Public Speaking")]
    PublicSpeaking = 272,
    #[serde(rename = "Race Walking")]
    RaceWalking = 274,
    #[serde(rename = "Driving")]
    Driving = 275,
    // Variants bellow are from openwhoop so to there is jump in numerical repr
    #[serde(rename = "Nap")]
    Nap = 1000,
}

impl ActivityType {
    pub fn icon_url(&self) -> &'static str {
        match self {
            ActivityType::Activity => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/unknown.png"
            }
            ActivityType::Running => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/running.png"
            }
            ActivityType::Cycling => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/cycling.png"
            }
            ActivityType::Baseball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/baseball.png"
            }
            ActivityType::Basketball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/basketball.png"
            }
            ActivityType::Rowing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/crew.png"
            }
            ActivityType::Fencing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/fencing.png"
            }
            ActivityType::FieldHockey => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/field_hockey.png"
            }
            ActivityType::Football => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/football.png"
            }
            ActivityType::Golf => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/golf.png"
            }
            ActivityType::IceHockey => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/ice_hockey.png"
            }
            ActivityType::Lacrosse => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/lacrosse.png"
            }
            ActivityType::Rugby => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/rugby.png"
            }
            ActivityType::Sailing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/sailing.png"
            }
            ActivityType::Skiing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/skiing.png"
            }
            ActivityType::Soccer => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/soccer.png"
            }
            ActivityType::Softball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/softball.png"
            }
            ActivityType::Squash => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/squash.png"
            }
            ActivityType::Swimming => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/swimming_diving.png"
            }
            ActivityType::Tennis => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/tennis.png"
            }
            ActivityType::TrackField => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/track_and_field.png"
            }
            ActivityType::Volleyball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/volleyball.png"
            }
            ActivityType::WaterPolo => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/water_polo.png"
            }
            ActivityType::Wrestling => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/wrestling.png"
            }
            ActivityType::Boxing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/boxing.png"
            }
            ActivityType::Dance => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/dance.png"
            }
            ActivityType::Pilates => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/pilates.png"
            }
            ActivityType::Yoga => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/yoga.png"
            }
            ActivityType::Weightlifting => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/weightlifting.png"
            }
            ActivityType::Canoeing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/canoeing.png"
            }
            ActivityType::CrossCountrySkiing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/cross_country_skiing.png"
            }
            ActivityType::FunctionalFitness => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/functional_fitness.png"
            }
            ActivityType::Duathlon => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/duathlon.png"
            }
            ActivityType::MachineWorkout => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/machine_workout.png"
            }
            ActivityType::Gymnastics => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/gymnastics.png"
            }
            ActivityType::HikingRucking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/hiking.png"
            }
            ActivityType::HorsebackRiding => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/horseback_riding.png"
            }
            ActivityType::Jogging => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/jogging.png"
            }
            ActivityType::Kayaking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/kayaking.png"
            }
            ActivityType::MartialArts => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/martial_arts.png"
            }
            ActivityType::MountainBiking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/mountain_biking.png"
            }
            ActivityType::ObstacleRacing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/obstacle_racing.png"
            }
            ActivityType::Powerlifting => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/powerlifting.png"
            }
            ActivityType::RockClimbing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/rock_climbing.png"
            }
            ActivityType::Paddleboarding => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/SUP.png"
            }
            ActivityType::Triathlon => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/triathlon.png"
            }
            ActivityType::Walking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/walking.png"
            }
            ActivityType::Surfing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/surfing.png"
            }
            ActivityType::Elliptical => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/elliptical.png"
            }
            ActivityType::Stairmaster => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/stairmaster.png"
            }
            ActivityType::Plyometrics => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/plyometrics.png"
            }
            ActivityType::Spinning => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/spinning.png"
            }
            ActivityType::Sex => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/sex.png"
            }
            ActivityType::Meditation => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/meditation.png"
            }
            ActivityType::Other => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/other.png"
            }
            ActivityType::PitPractice => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/pitstop.png"
            }
            ActivityType::Diving => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/diving.png"
            }
            ActivityType::OperationsTactical => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/tactical_ops.png"
            }
            ActivityType::OperationsMedical => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/medical_ops.png"
            }
            ActivityType::OperationsFlying => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/flying_ops.png"
            }
            ActivityType::OperationsWater => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/water_ops.png"
            }
            ActivityType::Ultimate => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/ultimate_frisbee.png"
            }
            ActivityType::Climber => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/climber.png"
            }
            ActivityType::JumpingRope => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/jumping_rope.png"
            }
            ActivityType::AustralianRulesFootball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/australian_football.png"
            }
            ActivityType::Skateboarding => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/skateboarding.png"
            }
            ActivityType::Coaching => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/coaching.png"
            }
            ActivityType::IceBath => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/ice_bath.png"
            }
            ActivityType::Commuting => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/commuting.png"
            }
            ActivityType::Gaming => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/gaming.png"
            }
            ActivityType::Snowboarding => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/snowboarding.png"
            }
            ActivityType::Motocross => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/motocross.png"
            }
            ActivityType::Caddying => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/caddying.png"
            }
            ActivityType::ObstacleCourseRacing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/obstacle-course-racing.png"
            }
            ActivityType::MotorRacing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/motor-racing.png"
            }
            ActivityType::Hiit => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/hiit.png"
            }
            ActivityType::Spin => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/spin.png"
            }
            ActivityType::JiuJitsu => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/jiu-jitsu.png"
            }
            ActivityType::ManualLabor => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/manual-labor.png"
            }
            ActivityType::Cricket => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/cricket.png"
            }
            ActivityType::Pickleball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/pickleball.png"
            }
            ActivityType::InlineSkating => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/rollerblading.png"
            }
            ActivityType::BoxFitness => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/crossfit.png"
            }
            ActivityType::Spikeball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/spikeball.png"
            }
            ActivityType::WheelchairPushing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/wheelchair_pushing.png"
            }
            ActivityType::PaddleTennis => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/paddle_tennis.png"
            }
            ActivityType::Barre => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/barre.png"
            }
            ActivityType::StagePerformance => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/stage-performance.png"
            }
            ActivityType::HighStressWork => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/high-stress-work.png"
            }
            ActivityType::Parkour => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/parkour.png"
            }
            ActivityType::GaelicFootball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/gaelic-football.png"
            }
            ActivityType::HurlingCamogie => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/hurling-camogie.png"
            }
            ActivityType::CircusArts => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/circus-arts.png"
            }
            ActivityType::ResonanceFrequencyBreathing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/unknown.png"
            }
            ActivityType::MassageTherapy => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/massage_therapy.png"
            }
            ActivityType::StrengthTrainer => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/weightlifting.png"
            }
            ActivityType::WatchingSports => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/spectating.png"
            }
            ActivityType::AssaultBike => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/assault_bike.png"
            }
            ActivityType::Kickboxing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/kickboxing.png"
            }
            ActivityType::Stretching => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/stretching.png"
            }
            ActivityType::OtherRecovery => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/other.png"
            }
            ActivityType::TableTennisPingPong => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/ping_pong.png"
            }
            ActivityType::Badminton => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/badminton.png"
            }
            ActivityType::Netball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/netball.png"
            }
            ActivityType::Sauna => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/sauna.png"
            }
            ActivityType::DiscGolf => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/disc_golf.png"
            }
            ActivityType::YardWorkGardening => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/yard_work.png"
            }
            ActivityType::AirCompression => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/air_compression.png"
            }
            ActivityType::PercussiveMassage => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/percussive_massage.png"
            }
            ActivityType::Paintball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/paintball.png"
            }
            ActivityType::IceSkating => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/ice_skating.png"
            }
            ActivityType::Handball => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/handball.png"
            }
            ActivityType::PercussiveMassageHypervolt => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/percussive_massage.png"
            }
            ActivityType::AirCompressionNormatec => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/air_compression.png"
            }
            ActivityType::IncreaseRelaxation => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/increase_relaxation.png"
            }
            ActivityType::IncreaseAlertness => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/increase_alertness.png"
            }
            ActivityType::Breathwork => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/breathwork_lungs.png"
            }
            ActivityType::NonSleepDeepRest => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/non-sleep-deep-rest.png"
            }
            ActivityType::SteamRoom => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/steam-room.png"
            }
            ActivityType::F45Training => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/F45.png"
            }
            ActivityType::Padel => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/padel.png"
            }
            ActivityType::BarryS => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/barrys.png"
            }
            ActivityType::DedicatedParenting => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/dedicated_parenting.png"
            }
            ActivityType::StrollerWalking => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/stroller_walking.png"
            }
            ActivityType::StrollerJogging => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/stroller_jogging.png"
            }
            ActivityType::Toddlerwearing => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/toddler_wearing.png"
            }
            ActivityType::Babywearing => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/baby_wearing.png"
            }
            ActivityType::PlayingWithChild => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/playing_with_child.png"
            }
            ActivityType::CuddlingWithChild => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/cuddling_with_child.png"
            }
            ActivityType::Barre3 => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/barre3.png"
            }
            ActivityType::HotYoga => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/hot_yoga.png"
            }
            ActivityType::StadiumSteps => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/stadium-steps.png"
            }
            ActivityType::Polo => {
                "https://s3.us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/polo.png"
            }
            ActivityType::MusicalPerformance => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/musical-performance.png"
            }
            ActivityType::KiteBoarding => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/kiteboarding.png"
            }
            ActivityType::RestorativeYoga => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/restorative-yoga.png"
            }
            ActivityType::DogWalking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/dog-walking.png"
            }
            ActivityType::WaterSkiing => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/water-skiing.png"
            }
            ActivityType::Wakeboarding => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/wakeboarding.png"
            }
            ActivityType::Cooking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/cooking.png"
            }
            ActivityType::Cleaning => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/cleaning.png"
            }
            ActivityType::WarmBath => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/warm-bath.png"
            }
            ActivityType::PublicSpeaking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/public-speaking.png"
            }
            ActivityType::RaceWalking => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/race-walking.png"
            }
            ActivityType::Driving => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/driving.png"
            }
            ActivityType::Nap => {
                "https://s3-us-west-2.amazonaws.com/icons.whoop.com/mobile/activities/nap.png"
            }
        }
    }

    pub fn category(&self) -> Category {
        match self {
            ActivityType::Activity => Category::CardioVascular,
            ActivityType::Running => Category::CardioVascular,
            ActivityType::Cycling => Category::CardioVascular,
            ActivityType::Baseball => Category::NonCardio,
            ActivityType::Basketball => Category::CardioVascular,
            ActivityType::Rowing => Category::CardioVascular,
            ActivityType::Fencing => Category::CardioVascular,
            ActivityType::FieldHockey => Category::CardioVascular,
            ActivityType::Football => Category::CardioVascular,
            ActivityType::Golf => Category::NonCardio,
            ActivityType::IceHockey => Category::CardioVascular,
            ActivityType::Lacrosse => Category::CardioVascular,
            ActivityType::Rugby => Category::CardioVascular,
            ActivityType::Sailing => Category::NonCardio,
            ActivityType::Skiing => Category::NonCardio,
            ActivityType::Soccer => Category::CardioVascular,
            ActivityType::Softball => Category::NonCardio,
            ActivityType::Squash => Category::CardioVascular,
            ActivityType::Swimming => Category::CardioVascular,
            ActivityType::Tennis => Category::CardioVascular,
            ActivityType::TrackField => Category::CardioVascular,
            ActivityType::Volleyball => Category::CardioVascular,
            ActivityType::WaterPolo => Category::CardioVascular,
            ActivityType::Wrestling => Category::CardioVascular,
            ActivityType::Boxing => Category::CardioVascular,
            ActivityType::Dance => Category::CardioVascular,
            ActivityType::Pilates => Category::NonCardio,
            ActivityType::Yoga => Category::NonCardio,
            ActivityType::Weightlifting => Category::Muscular,
            ActivityType::Canoeing => Category::CardioVascular,
            ActivityType::CrossCountrySkiing => Category::CardioVascular,
            ActivityType::FunctionalFitness => Category::CardioVascular,
            ActivityType::Duathlon => Category::CardioVascular,
            ActivityType::MachineWorkout => Category::NonCardio,
            ActivityType::Gymnastics => Category::NonCardio,
            ActivityType::HikingRucking => Category::CardioVascular,
            ActivityType::HorsebackRiding => Category::NonCardio,
            ActivityType::Jogging => Category::CardioVascular,
            ActivityType::Kayaking => Category::CardioVascular,
            ActivityType::MartialArts => Category::CardioVascular,
            ActivityType::MountainBiking => Category::CardioVascular,
            ActivityType::ObstacleRacing => Category::CardioVascular,
            ActivityType::Powerlifting => Category::Muscular,
            ActivityType::RockClimbing => Category::NonCardio,
            ActivityType::Paddleboarding => Category::CardioVascular,
            ActivityType::Triathlon => Category::CardioVascular,
            ActivityType::Walking => Category::NonCardio,
            ActivityType::Surfing => Category::CardioVascular,
            ActivityType::Elliptical => Category::CardioVascular,
            ActivityType::Stairmaster => Category::CardioVascular,
            ActivityType::Plyometrics => Category::CardioVascular,
            ActivityType::Spinning => Category::CardioVascular,
            ActivityType::Sex => Category::CardioVascular,
            ActivityType::Meditation => Category::Restorative,
            ActivityType::Other => Category::CardioVascular,
            ActivityType::PitPractice => Category::NonCardio,
            ActivityType::Diving => Category::NonCardio,
            ActivityType::OperationsTactical => Category::NonCardio,
            ActivityType::OperationsMedical => Category::NonCardio,
            ActivityType::OperationsFlying => Category::NonCardio,
            ActivityType::OperationsWater => Category::NonCardio,
            ActivityType::Ultimate => Category::CardioVascular,
            ActivityType::Climber => Category::CardioVascular,
            ActivityType::JumpingRope => Category::CardioVascular,
            ActivityType::AustralianRulesFootball => Category::CardioVascular,
            ActivityType::Skateboarding => Category::CardioVascular,
            ActivityType::Coaching => Category::NonCardio,
            ActivityType::IceBath => Category::Restorative,
            ActivityType::Commuting => Category::CardioVascular,
            ActivityType::Gaming => Category::NonCardio,
            ActivityType::Snowboarding => Category::CardioVascular,
            ActivityType::Motocross => Category::CardioVascular,
            ActivityType::Caddying => Category::CardioVascular,
            ActivityType::ObstacleCourseRacing => Category::CardioVascular,
            ActivityType::MotorRacing => Category::CardioVascular,
            ActivityType::Hiit => Category::CardioVascular,
            ActivityType::Spin => Category::CardioVascular,
            ActivityType::JiuJitsu => Category::NonCardio,
            ActivityType::ManualLabor => Category::NonCardio,
            ActivityType::Cricket => Category::NonCardio,
            ActivityType::Pickleball => Category::CardioVascular,
            ActivityType::InlineSkating => Category::CardioVascular,
            ActivityType::BoxFitness => Category::CardioVascular,
            ActivityType::Spikeball => Category::CardioVascular,
            ActivityType::WheelchairPushing => Category::CardioVascular,
            ActivityType::PaddleTennis => Category::CardioVascular,
            ActivityType::Barre => Category::NonCardio,
            ActivityType::StagePerformance => Category::NonCardio,
            ActivityType::HighStressWork => Category::NonCardio,
            ActivityType::Parkour => Category::CardioVascular,
            ActivityType::GaelicFootball => Category::CardioVascular,
            ActivityType::HurlingCamogie => Category::CardioVascular,
            ActivityType::CircusArts => Category::NonCardio,
            ActivityType::ResonanceFrequencyBreathing => Category::Restorative,
            ActivityType::MassageTherapy => Category::Restorative,
            ActivityType::StrengthTrainer => Category::Muscular,
            ActivityType::WatchingSports => Category::NonCardio,
            ActivityType::AssaultBike => Category::CardioVascular,
            ActivityType::Kickboxing => Category::CardioVascular,
            ActivityType::Stretching => Category::Restorative,
            ActivityType::OtherRecovery => Category::Restorative,
            ActivityType::TableTennisPingPong => Category::NonCardio,
            ActivityType::Badminton => Category::CardioVascular,
            ActivityType::Netball => Category::CardioVascular,
            ActivityType::Sauna => Category::Restorative,
            ActivityType::DiscGolf => Category::CardioVascular,
            ActivityType::YardWorkGardening => Category::CardioVascular,
            ActivityType::AirCompression => Category::Restorative,
            ActivityType::PercussiveMassage => Category::Restorative,
            ActivityType::Paintball => Category::CardioVascular,
            ActivityType::IceSkating => Category::CardioVascular,
            ActivityType::Handball => Category::CardioVascular,
            ActivityType::PercussiveMassageHypervolt => Category::Restorative,
            ActivityType::AirCompressionNormatec => Category::Restorative,
            ActivityType::IncreaseRelaxation => Category::Restorative,
            ActivityType::IncreaseAlertness => Category::Restorative,
            ActivityType::Breathwork => Category::Restorative,
            ActivityType::NonSleepDeepRest => Category::Restorative,
            ActivityType::SteamRoom => Category::Restorative,
            ActivityType::F45Training => Category::CardioVascular,
            ActivityType::Padel => Category::CardioVascular,
            ActivityType::BarryS => Category::CardioVascular,
            ActivityType::DedicatedParenting => Category::CardioVascular,
            ActivityType::StrollerWalking => Category::CardioVascular,
            ActivityType::StrollerJogging => Category::CardioVascular,
            ActivityType::Toddlerwearing => Category::CardioVascular,
            ActivityType::Babywearing => Category::CardioVascular,
            ActivityType::PlayingWithChild => Category::Restorative,
            ActivityType::CuddlingWithChild => Category::Restorative,
            ActivityType::Barre3 => Category::NonCardio,
            ActivityType::HotYoga => Category::NonCardio,
            ActivityType::StadiumSteps => Category::CardioVascular,
            ActivityType::Polo => Category::CardioVascular,
            ActivityType::MusicalPerformance => Category::CardioVascular,
            ActivityType::KiteBoarding => Category::CardioVascular,
            ActivityType::RestorativeYoga => Category::Restorative,
            ActivityType::DogWalking => Category::CardioVascular,
            ActivityType::WaterSkiing => Category::CardioVascular,
            ActivityType::Wakeboarding => Category::CardioVascular,
            ActivityType::Cooking => Category::CardioVascular,
            ActivityType::Cleaning => Category::CardioVascular,
            ActivityType::WarmBath => Category::Restorative,
            ActivityType::PublicSpeaking => Category::CardioVascular,
            ActivityType::RaceWalking => Category::CardioVascular,
            ActivityType::Driving => Category::CardioVascular,
            ActivityType::Nap => Category::Restorative,
        }
    }
}

impl Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ActivityType::Activity => "Activity",
            ActivityType::Running => "Running",
            ActivityType::Cycling => "Cycling",
            ActivityType::Baseball => "Baseball",
            ActivityType::Basketball => "Basketball",
            ActivityType::Rowing => "Rowing",
            ActivityType::Fencing => "Fencing",
            ActivityType::FieldHockey => "Field Hockey",
            ActivityType::Football => "Football",
            ActivityType::Golf => "Golf",
            ActivityType::IceHockey => "Ice Hockey",
            ActivityType::Lacrosse => "Lacrosse",
            ActivityType::Rugby => "Rugby",
            ActivityType::Sailing => "Sailing",
            ActivityType::Skiing => "Skiing",
            ActivityType::Soccer => "Soccer",
            ActivityType::Softball => "Softball",
            ActivityType::Squash => "Squash",
            ActivityType::Swimming => "Swimming",
            ActivityType::Tennis => "Tennis",
            ActivityType::TrackField => "Track & Field",
            ActivityType::Volleyball => "Volleyball",
            ActivityType::WaterPolo => "Water Polo",
            ActivityType::Wrestling => "Wrestling",
            ActivityType::Boxing => "Boxing",
            ActivityType::Dance => "Dance",
            ActivityType::Pilates => "Pilates",
            ActivityType::Yoga => "Yoga",
            ActivityType::Weightlifting => "Weightlifting",
            ActivityType::Canoeing => "Canoeing",
            ActivityType::CrossCountrySkiing => "Cross Country Skiing",
            ActivityType::FunctionalFitness => "Functional Fitness",
            ActivityType::Duathlon => "Duathlon",
            ActivityType::MachineWorkout => "Machine Workout",
            ActivityType::Gymnastics => "Gymnastics",
            ActivityType::HikingRucking => "Hiking/Rucking",
            ActivityType::HorsebackRiding => "Horseback Riding",
            ActivityType::Jogging => "Jogging",
            ActivityType::Kayaking => "Kayaking",
            ActivityType::MartialArts => "Martial Arts",
            ActivityType::MountainBiking => "Mountain Biking",
            ActivityType::ObstacleRacing => "Obstacle Racing",
            ActivityType::Powerlifting => "Powerlifting",
            ActivityType::RockClimbing => "Rock Climbing",
            ActivityType::Paddleboarding => "Paddleboarding",
            ActivityType::Triathlon => "Triathlon",
            ActivityType::Walking => "Walking",
            ActivityType::Surfing => "Surfing",
            ActivityType::Elliptical => "Elliptical",
            ActivityType::Stairmaster => "Stairmaster",
            ActivityType::Plyometrics => "Plyometrics",
            ActivityType::Spinning => "Spinning",
            ActivityType::Sex => "Sex",
            ActivityType::Meditation => "Meditation",
            ActivityType::Other => "Other",
            ActivityType::PitPractice => "Pit Practice",
            ActivityType::Diving => "Diving",
            ActivityType::OperationsTactical => "Operations - Tactical",
            ActivityType::OperationsMedical => "Operations - Medical",
            ActivityType::OperationsFlying => "Operations - Flying",
            ActivityType::OperationsWater => "Operations - Water",
            ActivityType::Ultimate => "Ultimate",
            ActivityType::Climber => "Climber",
            ActivityType::JumpingRope => "Jumping Rope",
            ActivityType::AustralianRulesFootball => "Australian Rules Football",
            ActivityType::Skateboarding => "Skateboarding",
            ActivityType::Coaching => "Coaching",
            ActivityType::IceBath => "Ice Bath",
            ActivityType::Commuting => "Commuting",
            ActivityType::Gaming => "Gaming",
            ActivityType::Snowboarding => "Snowboarding",
            ActivityType::Motocross => "Motocross",
            ActivityType::Caddying => "Caddying",
            ActivityType::ObstacleCourseRacing => "Obstacle Course Racing",
            ActivityType::MotorRacing => "Motor Racing",
            ActivityType::Hiit => "HIIT",
            ActivityType::Spin => "Spin",
            ActivityType::JiuJitsu => "Jiu Jitsu",
            ActivityType::ManualLabor => "Manual Labor",
            ActivityType::Cricket => "Cricket",
            ActivityType::Pickleball => "Pickleball",
            ActivityType::InlineSkating => "Inline Skating",
            ActivityType::BoxFitness => "Box Fitness",
            ActivityType::Spikeball => "Spikeball",
            ActivityType::WheelchairPushing => "Wheelchair Pushing",
            ActivityType::PaddleTennis => "Paddle Tennis",
            ActivityType::Barre => "Barre",
            ActivityType::StagePerformance => "Stage Performance",
            ActivityType::HighStressWork => "High Stress Work",
            ActivityType::Parkour => "Parkour",
            ActivityType::GaelicFootball => "Gaelic Football",
            ActivityType::HurlingCamogie => "Hurling/Camogie",
            ActivityType::CircusArts => "Circus Arts",
            ActivityType::ResonanceFrequencyBreathing => "Resonance Frequency Breathing",
            ActivityType::MassageTherapy => "Massage Therapy",
            ActivityType::StrengthTrainer => "Strength Trainer",
            ActivityType::WatchingSports => "Watching Sports",
            ActivityType::AssaultBike => "Assault Bike",
            ActivityType::Kickboxing => "Kickboxing",
            ActivityType::Stretching => "Stretching",
            ActivityType::OtherRecovery => "Other - Recovery",
            ActivityType::TableTennisPingPong => "Table Tennis/Ping Pong",
            ActivityType::Badminton => "Badminton",
            ActivityType::Netball => "Netball",
            ActivityType::Sauna => "Sauna",
            ActivityType::DiscGolf => "Disc Golf",
            ActivityType::YardWorkGardening => "Yard Work/Gardening",
            ActivityType::AirCompression => "Air Compression",
            ActivityType::PercussiveMassage => "Percussive Massage",
            ActivityType::Paintball => "Paintball",
            ActivityType::IceSkating => "Ice Skating",
            ActivityType::Handball => "Handball",
            ActivityType::PercussiveMassageHypervolt => "Percussive Massage (Hypervolt)",
            ActivityType::AirCompressionNormatec => "Air Compression (Normatec)",
            ActivityType::IncreaseRelaxation => "Increase Relaxation",
            ActivityType::IncreaseAlertness => "Increase Alertness",
            ActivityType::Breathwork => "Breathwork",
            ActivityType::NonSleepDeepRest => "Non-Sleep Deep Rest",
            ActivityType::SteamRoom => "Steam Room",
            ActivityType::F45Training => "F45 Training",
            ActivityType::Padel => "Padel",
            ActivityType::BarryS => "Barry's",
            ActivityType::DedicatedParenting => "Dedicated Parenting",
            ActivityType::StrollerWalking => "Stroller Walking",
            ActivityType::StrollerJogging => "Stroller Jogging",
            ActivityType::Toddlerwearing => "Toddlerwearing",
            ActivityType::Babywearing => "Babywearing",
            ActivityType::PlayingWithChild => "Playing with Child",
            ActivityType::CuddlingWithChild => "Cuddling with Child",
            ActivityType::Barre3 => "Barre3",
            ActivityType::HotYoga => "Hot Yoga",
            ActivityType::StadiumSteps => "Stadium Steps",
            ActivityType::Polo => "Polo",
            ActivityType::MusicalPerformance => "Musical Performance",
            ActivityType::KiteBoarding => "Kite Boarding",
            ActivityType::RestorativeYoga => "Restorative Yoga",
            ActivityType::DogWalking => "Dog Walking",
            ActivityType::WaterSkiing => "Water Skiing",
            ActivityType::Wakeboarding => "Wakeboarding",
            ActivityType::Cooking => "Cooking",
            ActivityType::Cleaning => "Cleaning",
            ActivityType::WarmBath => "Warm Bath",
            ActivityType::PublicSpeaking => "Public Speaking",
            ActivityType::RaceWalking => "Race Walking",
            ActivityType::Driving => "Driving",
            ActivityType::Nap => "Nap",
        };

        write!(f, "{}", s)
    }
}

impl FromStr for ActivityType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Activity" => Ok(ActivityType::Activity),
            "Running" => Ok(ActivityType::Running),
            "Cycling" => Ok(ActivityType::Cycling),
            "Baseball" => Ok(ActivityType::Baseball),
            "Basketball" => Ok(ActivityType::Basketball),
            "Rowing" => Ok(ActivityType::Rowing),
            "Fencing" => Ok(ActivityType::Fencing),
            "Field Hockey" => Ok(ActivityType::FieldHockey),
            "Football" => Ok(ActivityType::Football),
            "Golf" => Ok(ActivityType::Golf),
            "Ice Hockey" => Ok(ActivityType::IceHockey),
            "Lacrosse" => Ok(ActivityType::Lacrosse),
            "Rugby" => Ok(ActivityType::Rugby),
            "Sailing" => Ok(ActivityType::Sailing),
            "Skiing" => Ok(ActivityType::Skiing),
            "Soccer" => Ok(ActivityType::Soccer),
            "Softball" => Ok(ActivityType::Softball),
            "Squash" => Ok(ActivityType::Squash),
            "Swimming" => Ok(ActivityType::Swimming),
            "Tennis" => Ok(ActivityType::Tennis),
            "Track & Field" => Ok(ActivityType::TrackField),
            "Volleyball" => Ok(ActivityType::Volleyball),
            "Water Polo" => Ok(ActivityType::WaterPolo),
            "Wrestling" => Ok(ActivityType::Wrestling),
            "Boxing" => Ok(ActivityType::Boxing),
            "Dance" => Ok(ActivityType::Dance),
            "Pilates" => Ok(ActivityType::Pilates),
            "Yoga" => Ok(ActivityType::Yoga),
            "Weightlifting" => Ok(ActivityType::Weightlifting),
            "Canoeing" => Ok(ActivityType::Canoeing),
            "Cross Country Skiing" => Ok(ActivityType::CrossCountrySkiing),
            "Functional Fitness" => Ok(ActivityType::FunctionalFitness),
            "Duathlon" => Ok(ActivityType::Duathlon),
            "Machine Workout" => Ok(ActivityType::MachineWorkout),
            "Gymnastics" => Ok(ActivityType::Gymnastics),
            "Hiking/Rucking" => Ok(ActivityType::HikingRucking),
            "Horseback Riding" => Ok(ActivityType::HorsebackRiding),
            "Jogging" => Ok(ActivityType::Jogging),
            "Kayaking" => Ok(ActivityType::Kayaking),
            "Martial Arts" => Ok(ActivityType::MartialArts),
            "Mountain Biking" => Ok(ActivityType::MountainBiking),
            "Obstacle Racing" => Ok(ActivityType::ObstacleRacing),
            "Powerlifting" => Ok(ActivityType::Powerlifting),
            "Rock Climbing" => Ok(ActivityType::RockClimbing),
            "Paddleboarding" => Ok(ActivityType::Paddleboarding),
            "Triathlon" => Ok(ActivityType::Triathlon),
            "Walking" => Ok(ActivityType::Walking),
            "Surfing" => Ok(ActivityType::Surfing),
            "Elliptical" => Ok(ActivityType::Elliptical),
            "Stairmaster" => Ok(ActivityType::Stairmaster),
            "Plyometrics" => Ok(ActivityType::Plyometrics),
            "Spinning" => Ok(ActivityType::Spinning),
            "Sex" => Ok(ActivityType::Sex),
            "Meditation" => Ok(ActivityType::Meditation),
            "Other" => Ok(ActivityType::Other),
            "Pit Practice" => Ok(ActivityType::PitPractice),
            "Diving" => Ok(ActivityType::Diving),
            "Operations - Tactical" => Ok(ActivityType::OperationsTactical),
            "Operations - Medical" => Ok(ActivityType::OperationsMedical),
            "Operations - Flying" => Ok(ActivityType::OperationsFlying),
            "Operations - Water" => Ok(ActivityType::OperationsWater),
            "Ultimate" => Ok(ActivityType::Ultimate),
            "Climber" => Ok(ActivityType::Climber),
            "Jumping Rope" => Ok(ActivityType::JumpingRope),
            "Australian Rules Football" => Ok(ActivityType::AustralianRulesFootball),
            "Skateboarding" => Ok(ActivityType::Skateboarding),
            "Coaching" => Ok(ActivityType::Coaching),
            "Ice Bath" => Ok(ActivityType::IceBath),
            "Commuting" => Ok(ActivityType::Commuting),
            "Gaming" => Ok(ActivityType::Gaming),
            "Snowboarding" => Ok(ActivityType::Snowboarding),
            "Motocross" => Ok(ActivityType::Motocross),
            "Caddying" => Ok(ActivityType::Caddying),
            "Obstacle Course Racing" => Ok(ActivityType::ObstacleCourseRacing),
            "Motor Racing" => Ok(ActivityType::MotorRacing),
            "HIIT" => Ok(ActivityType::Hiit),
            "Spin" => Ok(ActivityType::Spin),
            "Jiu Jitsu" => Ok(ActivityType::JiuJitsu),
            "Manual Labor" => Ok(ActivityType::ManualLabor),
            "Cricket" => Ok(ActivityType::Cricket),
            "Pickleball" => Ok(ActivityType::Pickleball),
            "Inline Skating" => Ok(ActivityType::InlineSkating),
            "Box Fitness" => Ok(ActivityType::BoxFitness),
            "Spikeball" => Ok(ActivityType::Spikeball),
            "Wheelchair Pushing" => Ok(ActivityType::WheelchairPushing),
            "Paddle Tennis" => Ok(ActivityType::PaddleTennis),
            "Barre" => Ok(ActivityType::Barre),
            "Stage Performance" => Ok(ActivityType::StagePerformance),
            "High Stress Work" => Ok(ActivityType::HighStressWork),
            "Parkour" => Ok(ActivityType::Parkour),
            "Gaelic Football" => Ok(ActivityType::GaelicFootball),
            "Hurling/Camogie" => Ok(ActivityType::HurlingCamogie),
            "Circus Arts" => Ok(ActivityType::CircusArts),
            "Resonance Frequency Breathing" => Ok(ActivityType::ResonanceFrequencyBreathing),
            "Massage Therapy" => Ok(ActivityType::MassageTherapy),
            "Strength Trainer" => Ok(ActivityType::StrengthTrainer),
            "Watching Sports" => Ok(ActivityType::WatchingSports),
            "Assault Bike" => Ok(ActivityType::AssaultBike),
            "Kickboxing" => Ok(ActivityType::Kickboxing),
            "Stretching" => Ok(ActivityType::Stretching),
            "Other - Recovery" => Ok(ActivityType::OtherRecovery),
            "Table Tennis/Ping Pong" => Ok(ActivityType::TableTennisPingPong),
            "Badminton" => Ok(ActivityType::Badminton),
            "Netball" => Ok(ActivityType::Netball),
            "Sauna" => Ok(ActivityType::Sauna),
            "Disc Golf" => Ok(ActivityType::DiscGolf),
            "Yard Work/Gardening" => Ok(ActivityType::YardWorkGardening),
            "Air Compression" => Ok(ActivityType::AirCompression),
            "Percussive Massage" => Ok(ActivityType::PercussiveMassage),
            "Paintball" => Ok(ActivityType::Paintball),
            "Ice Skating" => Ok(ActivityType::IceSkating),
            "Handball" => Ok(ActivityType::Handball),
            "Percussive Massage (Hypervolt)" => Ok(ActivityType::PercussiveMassageHypervolt),
            "Air Compression (Normatec)" => Ok(ActivityType::AirCompressionNormatec),
            "Increase Relaxation" => Ok(ActivityType::IncreaseRelaxation),
            "Increase Alertness" => Ok(ActivityType::IncreaseAlertness),
            "Breathwork" => Ok(ActivityType::Breathwork),
            "Non-Sleep Deep Rest" => Ok(ActivityType::NonSleepDeepRest),
            "Steam Room" => Ok(ActivityType::SteamRoom),
            "F45 Training" => Ok(ActivityType::F45Training),
            "Padel" => Ok(ActivityType::Padel),
            "Barry's" => Ok(ActivityType::BarryS),
            "Dedicated Parenting" => Ok(ActivityType::DedicatedParenting),
            "Stroller Walking" => Ok(ActivityType::StrollerWalking),
            "Stroller Jogging" => Ok(ActivityType::StrollerJogging),
            "Toddlerwearing" => Ok(ActivityType::Toddlerwearing),
            "Babywearing" => Ok(ActivityType::Babywearing),
            "Playing with Child" => Ok(ActivityType::PlayingWithChild),
            "Cuddling with Child" => Ok(ActivityType::CuddlingWithChild),
            "Barre3" => Ok(ActivityType::Barre3),
            "Hot Yoga" => Ok(ActivityType::HotYoga),
            "Stadium Steps" => Ok(ActivityType::StadiumSteps),
            "Polo" => Ok(ActivityType::Polo),
            "Musical Performance" => Ok(ActivityType::MusicalPerformance),
            "Kite Boarding" => Ok(ActivityType::KiteBoarding),
            "Restorative Yoga" => Ok(ActivityType::RestorativeYoga),
            "Dog Walking" => Ok(ActivityType::DogWalking),
            "Water Skiing" => Ok(ActivityType::WaterSkiing),
            "Wakeboarding" => Ok(ActivityType::Wakeboarding),
            "Cooking" => Ok(ActivityType::Cooking),
            "Cleaning" => Ok(ActivityType::Cleaning),
            "Warm Bath" => Ok(ActivityType::WarmBath),
            "Public Speaking" => Ok(ActivityType::PublicSpeaking),
            "Race Walking" => Ok(ActivityType::RaceWalking),
            "Driving" => Ok(ActivityType::Driving),
            "Nap" => Ok(ActivityType::Nap),
            _ => Err(()),
        }
    }
}

#[derive(Default)]
pub struct SearchActivityPeriods {
    pub from: Option<NaiveDateTime>,
    pub to: Option<NaiveDateTime>,
    pub activity: Option<ActivityType>,
}

impl SearchActivityPeriods {
    pub fn with_activity(self, activity: ActivityType) -> Self {
        Self {
            activity: Some(activity),
            ..self
        }
    }
}
