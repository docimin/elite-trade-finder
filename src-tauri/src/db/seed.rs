use super::Db;
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CommoditySeed {
    commodity_id: i64,
    symbol: String,
    display_name: String,
    category: Option<String>,
    #[serde(default)]
    is_rare: bool,
    #[serde(default)]
    is_illegal_hint: bool,
}

const SEED_JSON: &str = include_str!("../../data/commodities.json");

/// Known rare commodity symbols (EDCD canonical names). Used to back-fill the
/// `is_rare` flag on commodities auto-inserted by the ingestor.
const RARE_SYMBOLS: &[&str] = &[
    "AlacarakmoSkinArt", "AlbinoQuechuaMammoth", "AltairianSkin", "AnduligaFireWorks",
    "AnyNaCoffee", "ApaVietii", "BakedGreebles", "BankiAmphibiousLeather",
    "BastSnakeGin", "BelalansRayLeather", "BorasetaniPathogenetics", "BuckyballBeerMats",
    "BurnhamBileDistillate", "CD75CatCoffee", "CentauriMegaGin", "CeremonialHeikeTea",
    "CetiRabbits", "ChameleonCloth", "ChateauDeAegaeon", "ChiEridaniMarinePaste",
    "CoquimSpongiformVictuals", "CrystallineSpheres", "DamnaCarapaces", "DeltaPhoenicisPalms",
    "DeuringasTruffles", "DiabaRedwood", "DisoMaCorn", "EdenApplesOfAerial",
    "EleuThermals", "EranLeatherGoods", "EraninPearlWhisky", "EsusekuCaviar",
    "EthgrezeTeaBuds", "FujinTea", "GalactrvagsVirus", "GeawenDanceDust",
    "GerasianGueuzeBeer", "GilyaSignatureWeapons", "GiantIrukamaSnails", "GomanYauponCoffee",
    "HaidneMirrors", "HarmaSilverSeaRum", "HavingOlives", "HelvetitjPearls",
    "HipProto-Squid", "HolvaDuellingBlades", "HonestyPills", "HR7221Wheat",
    "IndiBourbon", "JadeiteRock", "JaquesQuinentianStill", "JaradharrePuzzlebox",
    "JarouaRice", "JotunMookah", "KachiriginLeaches", "KamitraCigars",
    "KamorinHistoricWeapons", "KaretiiCouture", "KinagoViolins", "KonggaAle",
    "KorroKungPellets", "LavianBrandy", "LeestianEvilJuice", "LFTVoidExtract",
    "LiveHecateSeaWorms", "LTTHyperSweet", "LyraeWeed", "MasterChefs",
    "MechucosHighTea", "MedbStarlube", "MomusBogSpaniel", "MotronaExperienceJelly",
    "MukusubiiChitinOs", "MulachiGiantFungus", "NeritusBerries", "NgadandariFireOpals",
    "NjangariSaddles", "NonEuclidianExotanks", "NoonaRumbleAle", "OchoengChillies",
    "OnionheadAlphaStrain", "OnionheadBetaStrain", "OnionheadGammaStrain",
    "OphiuchExinoArtefacts", "OrrerianViciousBrew", "PantaaPrayerSticks", "ParragereanPearls",
    "PavonisEarGrubs", "PlatinumAlloy", "RajukruStoves", "RapaBaoSnakeSkins",
    "RusaniOldSmokey", "SanumaDecorativeMeat", "SanumaMeat", "SaxonWine",
    "ShanscheeUsamhail", "SothisCrystallineGold", "SoontillRelics", "TaaffeiteGlass",
    "TanmarkTranquilTea", "TarachSpice", "TaurianChocolate", "TerraMaterBloodBores",
    "TheHuttonMug", "ThrutisCream", "TiegfriesSynthSilk", "TiolceWaste2PasteUnits",
    "TransgenicOnionHead", "UszaianTreeGrub", "UtgaroarMillennialEggs", "UzumokuLowGWings",
    "V_HerculisBodyRub", "VanayequiCeratomorphaFur", "VegaSlimweed", "VidavantianLace",
    "VolkhabBeeDrones", "WheemeteWheatCakes", "WitchhaulKobeBeef", "WolfFesh",
    "WuthieloKuFroth", "XiheCompanions", "YasoKondiLeaf", "ZeesszeAntGrubGlue",
];

pub async fn commodities(db: &Db) -> Result<usize> {
    let items: Vec<CommoditySeed> =
        serde_json::from_str(SEED_JSON).context("parsing commodities.json")?;
    let mut inserted = 0;
    for c in items {
        match db {
            Db::Sqlite(pool) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO commodities (commodity_id, symbol, display_name, category, is_rare, is_illegal_hint) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(c.commodity_id)
                .bind(&c.symbol)
                .bind(&c.display_name)
                .bind(c.category.as_deref())
                .bind(c.is_rare as i32)
                .bind(c.is_illegal_hint as i32)
                .execute(pool)
                .await?;
            }
            Db::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO commodities (commodity_id, symbol, display_name, category, is_rare, is_illegal_hint) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (commodity_id) DO NOTHING",
                )
                .bind(c.commodity_id as i32)
                .bind(&c.symbol)
                .bind(&c.display_name)
                .bind(c.category.as_deref())
                .bind(c.is_rare)
                .bind(c.is_illegal_hint)
                .execute(pool)
                .await?;
            }
        }
        inserted += 1;
    }

    // Back-fill is_rare on commodities the ingestor auto-inserted but didn't flag.
    for symbol in RARE_SYMBOLS {
        match db {
            Db::Sqlite(pool) => {
                sqlx::query("UPDATE commodities SET is_rare = 1 WHERE symbol = ? AND is_rare = 0")
                    .bind(symbol)
                    .execute(pool)
                    .await?;
            }
            Db::Postgres(pool) => {
                sqlx::query("UPDATE commodities SET is_rare = TRUE WHERE symbol = $1 AND is_rare = FALSE")
                    .bind(symbol)
                    .execute(pool)
                    .await?;
            }
        }
    }

    Ok(inserted)
}
