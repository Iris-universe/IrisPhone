# IrisPhone

**Créer des apps iPhone sous Windows. Sans Mac, sans Xcode, sans compte payant.**

IrisPhone installe tout ce qu'il faut pour compiler des apps Swift pour iOS sur un PC Windows (ou Linux),
les signe avec ton Apple ID et les installe sur ton iPhone par câble USB, en une commande. Il est pensé pour
[Claude Code](https://claude.com/claude-code) : Claude écrit l'app, la compile, l'installe sur ton téléphone et
lit ses logs tout seul.

[English](README.md)

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/Iris-universe/IrisPhone/main/install.ps1 | iex
```

```sh
# Linux (Ubuntu/Debian)
curl -fsSL https://raw.githubusercontent.com/Iris-universe/IrisPhone/main/install.sh | sh
```

L'installateur met `irisphone` dans le PATH puis lance `irisphone setup`, qui guide pour la suite
(WSL, pilote USB d'Apple, Swift, SDK iOS, connexion Apple ID, Claude Code).

## Première app

```sh
irisphone new MonApp     # ou : irisphone new MonApp --template webview
cd MonApp
irisphone run            # compile, signe, installe et lance l'app sur l'iPhone
```

Seulement la première fois, sur l'iPhone : **Réglages > Général > VPN et gestion de l'appareil** > ton Apple ID
> **Faire confiance**, puis **Réglages > Confidentialité et sécurité > Mode développeur** > activé.

## Avec Claude Code

Chaque projet créé par `irisphone new` contient un `CLAUDE.md` (ce qui marche et ce qui ne marche pas sans
Xcode) et un `.mcp.json` qui donne à Claude les outils `irisphone_build`, `irisphone_run`, `irisphone_logs`,
`irisphone_doctor`, etc. Il suffit de lancer `claude` dans le dossier de l'app et de décrire ce que tu veux.

`irisphone setup` propose aussi d'enregistrer ces outils pour tous tes projets (`irisphone claude` le fait
plus tard). Un plugin Claude Code avec une skill iOS est disponible :

```
/plugin marketplace add Iris-universe/IrisPhone
/plugin install irisphone@irisphone
```

## Limites

- Windows 11 (22H2+) avec WSL 2, ou Ubuntu/Debian. Environ 12 Go une fois installé ; garder ~30 Go libres
  pendant l'installation.
- Le SDK iOS vient du `.xip` de Xcode que **tu** télécharges chez Apple (sa licence interdit de le redistribuer) ;
  `irisphone setup` ouvre la page et attend la fin du téléchargement.
- Apple ID gratuit : l'app expire au bout de **7 jours** (refaire `irisphone run`), 3 apps installées à la fois,
  10 nouveaux bundle IDs par semaine, pas de notifications push, d'iCloud ni d'App Groups. Un compte développeur
  payant (99 $/an) lève ces limites.
- Pas de simulateur iOS : on teste sur un vrai iPhone ou iPad.

Le reste de la documentation (commandes, fonctionnement, dépannage) est en anglais : [README.md](README.md),
[docs/](docs/).

## Licence

[MIT](LICENSE). Projet non affilié à Apple.
