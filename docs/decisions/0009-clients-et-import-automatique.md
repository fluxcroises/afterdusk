# 0009. Clients, séances et import automatique

Date : 2026-10-21 · Statut : **adoptée** (périmètre et règle réseau le 2026-10-21 ; détails tranchés le 2026-10-21, en fin de document)

## Contexte

L'utilisateur a posé deux questions le 2026-10-21 :

1. Afterdusk peut-il aider un photographe à organiser son travail, en liant un import à un client et à un type de séance (mariage, baptême, naissance…) ?
2. Les photos prises par un boîtier peuvent-elles arriver d'elles-mêmes dans Afterdusk et s'y afficher ?

Jusqu'ici, le cahier des charges n'avait que l'album comme unité d'organisation : un client ne pouvait apparaître que dans son nom. La connexion en direct au boîtier restait hors périmètre jusqu'à la V1, et la règle « aucun appel réseau hors tuiles de carte et vérification des mises à jour » écartait toute réception par le réseau. L'utilisateur accepte maintenant des exceptions à cette règle.

## Décisions

### Règle réseau : des exceptions listées

La règle absolue devient : aucun appel réseau hors des exceptions listées dans `CLAUDE.md` et le cahier des charges (§8). Chaque exception se désactive dans les Réglages et n'envoie ni photo ni donnée personnelle à un tiers sans action de l'utilisateur. Une nouvelle exception demande l'accord de l'utilisateur.

Exceptions au 2026-10-21 : tuiles de carte, vérification des mises à jour, réception FTP sur le réseau local (désactivée par défaut).

Le principe « Local et privé » (aucun compte, aucun cloud, analyses sur la machine) ne change pas. GeoNames et lensfun restent embarqués (décision 0004) : ils marchent hors ligne et ne pèsent que quelques Mo.

### Clients et séances (V0.4)

- Une table `clients` (nom, contact facultatif, notes) et, sur `albums`, l'id du client et le type de séance.
- Le type de séance vient d'une liste modifiable, fournie avec : mariage, baptême, naissance, portrait, événement.
- À l'import, le panneau de destination propose le client et le type de séance ; les modèles de rangement et de renommage gagnent `{client}` et `{seance}`.
- La Bibliothèque filtre et cherche par client et par type de séance ; la fiche d'un client liste ses albums.
- La galerie client de la V0.4 reprend le nom du client et réimporte ses choix dans le bon album.
- Les données des clients sont des données personnelles de tiers (RGPD) : le moins de champs possible, stockées seulement dans la base locale, jamais dans les XMP, et la suppression d'un client efface toutes ses données (les albums restent, sans client).
- Devis, factures, agenda et contrats restent hors périmètre : d'autres logiciels le font.

La fonction va en V0.4 parce qu'elle prolonge la galerie client. Elle demande une migration SQLite numérotée, comme toute évolution du schéma.

### Import automatique (V0.4)

Trois sources, toutes avec les garanties de l'import normal : copie vérifiée par somme de contrôle, sauvegarde si elle est configurée, rien d'écrit sur la source.

| Source            | Fonctionnement                                                                                                                                                            | Pourquoi                                                                                              |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Carte connue      | Option par carte : importer sans confirmation avec les derniers réglages ; une notification donne le nombre de photos ajoutées.                                           | Prolonge la détection des cartes de la V0.1, sans nouvelle dépendance.                                |
| Dossier surveillé | Afterdusk surveille un ou plusieurs dossiers, attend qu'un fichier soit complet (taille stable), le copie et l'affiche dans le tri.                                       | Marche avec toutes les marques : l'application du fabricant (USB ou Wi-Fi) dépose, Afterdusk importe. |
| Réception FTP     | Un serveur FTP limité au réseau local reçoit les envois des boîtiers qui savent transférer en FTP. Désactivé par défaut, identifiant et mot de passe propres à Afterdusk. | Pas d'application intermédiaire pour les boîtiers qui transfèrent en FTP par Wi-Fi ou Ethernet.       |

Un fichier reçu par FTP est d'abord écrit dans un dossier de réception, puis suit le chemin du dossier surveillé.

Pendant une séance, l'album en cours reste ouvert dans Trier : les nouvelles photos s'ajoutent à la fin sans changer la photo affichée.

### Ce qui reste hors périmètre

La connexion en direct au boîtier (déclenchement et réglages depuis Afterdusk) reste prévue après la V1. Les SDK des fabricants ne sont pas sous licence libre, donc incompatibles avec la GPL v3, et libgphoto2 fonctionne mal sous Windows. Le dossier surveillé couvre le besoin courant : voir ses photos arriver pendant la séance.

## Bibliothèques à valider par prototype

- Surveillance de dossiers : la crate `notify` (Windows et macOS). Licence et comportement sur un dossier réseau à vérifier.
- Serveur FTP : `libunftp` (Apache 2.0, compatible GPL v3), avec un stockage limité au dossier de réception. Prise en charge de FTPS à vérifier.

## Questions tranchées (2026-10-21)

Réponses de l'utilisateur, la V0.3 terminée :

1. **Version et ordre** : tout l'import automatique en V0.4, dans cet ordre : carte connue d'abord (elle prolonge la détection des cartes), puis dossier surveillé, puis réception FTP.
2. **Délai d'affichage** : une photo arrivée s'affiche dans le tri en moins de 5 secondes.
3. **FTP ou FTPS** : les deux sont acceptés, FTPS conseillé. En FTP simple, un avertissement rappelle que le mot de passe et les photos circulent en clair sur le réseau local.
4. **Fiche client** : nom, courriel et téléphone (tous deux facultatifs), notes. Ces deux données personnelles restent dans la base locale et disparaissent avec le client.
5. **Types de séance fournis** : mariage, baptême, naissance, portrait, événement, grossesse, famille, entreprise, produit, sport, concert. La liste reste modifiable.

## Planches contact (ajout du 2026-10-21)

Idée de l'utilisateur, ajoutée à l'étape Livrer de la V0.4 : une planche contact en PDF d'un album ou d'une collection, mise en page par le cœur avec printpdf (décision prise pour l'export d'inventaire). Choix de l'utilisateur :

- **Grille au choix** : 2 à 8 colonnes, A4 ou Lettre, portrait ou paysage ; par défaut 4 colonnes en A4 portrait.
- **Sous chaque vignette** : son numéro d'ordre (1, 2, 3…, pour qu'un client donne ses choix sans ambiguïté) et le nom du fichier.
- **En-tête** : nom de l'album ou de la collection, client et séance, date de la séance, photographe (identité des Réglages).
- **Filigrane** en option, désactivé par défaut, repris du filigrane texte des presets d'export.
- **Vignettes retouchées** : tirées des aperçus rendus, recadrage et réglages compris.
