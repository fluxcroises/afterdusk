# 0009. Clients, séances et import automatique

Date : 2026-10-21 · Statut : **adoptée** pour le périmètre et la règle réseau (demande de l'utilisateur du 2026-10-21) ; détails à trancher en fin de document

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

## Questions à trancher

1. **Version** : tout en V0.4, comme proposé, ou l'import automatique d'une carte connue plus tôt (juste après la V0.1, il ne demande presque rien) ?
2. **Délai d'affichage** : le critère proposé est « affichée dans le tri moins de 5 secondes après son arrivée », sur le modèle de la détection des cartes. Il te convient ?
3. **FTP ou FTPS** : en FTP simple, le mot de passe et les photos circulent en clair sur le réseau local. Exiger FTPS quand le boîtier le permet, ou accepter les deux ?
4. **Fiche client** : quels champs de contact (courriel, téléphone, aucun) ? Moins il y en a, moins il y a de données personnelles à protéger.
5. **Types de séance fournis** : la liste mariage, baptême, naissance, portrait, événement te va, ou faut-il en ajouter (grossesse, entreprise, sport…) ?
