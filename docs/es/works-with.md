---
title: Funciona con
layout: default
parent: Inicio
nav_order: 2.6
---

# Funciona con
{: .no_toc }

Aplicaciones y herramientas de terceros que funcionan junto con A2DPWB, según nos lo han contado sus autores o usuarios en los issues.

{: .warning }
> **No forman parte de A2DPWB**, y el proyecto A2DPWB no las crea, ni las prueba, ni les da soporte. Solo aparecen porque alguien informó de que funcionan con A2DPWB. No asumimos ninguna responsabilidad si una no funciona, deja de funcionar tras una actualización de cualquiera de los dos programas o causa algún problema. Consulte la licencia y la documentación propias de cada proyecto, e informe primero de los problemas de una herramienta al proyecto de esa herramienta.

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

## Aplicaciones y herramientas

| Herramienta | Qué hace con A2DPWB | Requisitos | Licencia | Informado por |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | Control de auriculares (batería, control de ruido, EQ, gestos) para OPPO, Sony, HUAWEI, Xiaomi y otros. Se conecta a la canalización del núcleo como segundo cliente: muestra el estado de la transmisión y el códec, inicia y detiene perfiles, ajusta el volumen AVRCP y ayuda con la asignación de WinUSB. Los ajustes de los auriculares a través del propio enlace Bluetooth de A2DPWB usan el paso directo RFCOMM | EncoDeck 1.11.0 o posterior. Paso directo RFCOMM: A2DPWB 2.0.2-beta.10 o posterior (hasta ahora solo probado en el emulador) | GPL-3.0 | Su autor, [#194](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/194) |

## Añadir una herramienta

¿Ha creado o encontrado una herramienta que funciona con A2DPWB? Cuéntenoslo en un [issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues) con su nombre, un enlace, su licencia, lo que hace con A2DPWB y las versiones que probó. Aparecer aquí no es una reseña ni una recomendación.

## Para autores de herramientas

Las herramientas se comunican con el núcleo por la misma canalización con nombre por usuario que la ventana de A2DPWB. El protocolo, los métodos pensados para herramientas, el paso directo RFCOMM y las reglas de compatibilidad (comprobación de funciones, el cambio de nombre de la canalización en 3.0.0) están en **[Integración (IPC)](integration)**. El protocolo y el núcleo aún pueden cambiar durante las betas de 2.0.2.
