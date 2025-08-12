theme-button-light = ☀ Claro
theme-button-dark = 🌙 Oscuro
theme-button-system = 💻 Sistema

# format "<language in native language> (<country in native language>)
language-es-ES = Español (España)
language-en-US = English (United States)

menu-top-level-file = Archivo
menu-item-quit = Salir

modal-errors-title = Errores - { $file }
modal-add-phase-title = Añadir fase - { $file }
modal-create-unit-assignment-title = Crear asignación de unidad - { $file }
modal-phase-placement-orderings-title = Ordenación de la colocación de fases - { $phase }
modal-manager-gerbers-title = Gestionar gerbers  - { $design }

toolbar-button-home = Inicio
toolbar-button-new-project = Nuevo proyecto
toolbar-button-open-project = Abrir proyecto
toolbar-button-new-pcb = Nuevo PCB
toolbar-button-open-pcb = Abrir PCB
toolbar-button-save = Guardar
toolbar-button-close-all = Cerrar todo

side-bar-header = Encabezado de la barra lateral
side-bar-footer = Pie de página de la barra lateral
side-bar-item-path = Ruta

project-toolbar-button-show-explorer = Mostrar explorador
project-toolbar-button-generate-artifacts = Generar artefactos
project-toolbar-button-refresh = Actualizar
project-toolbar-button-remove-unused-placements = Eliminar ubicaciones no utilizadas
project-toolbar-button-add-pcb = Añadir placa
project-toolbar-button-add-phase = Añadir fase

project-pcb-toolbar-button-create-unit-assignment = Crear asignacion de unidad
project-pcb-toolbar-button-show-pcb = Mostrar PCB

tab-label-home = Inicio
tab-label-new-project = Nuevo proyecto
tab-label-new-pcb = New PCB

home-banner = MakerPnP - Planner
home-checkbox-label-show-on-startup = Mostrar al inicio

new-project-banner = Nuevo proyecto
form-new-project-input-name = Nombre del proyecto
form-new-project-input-directory = Directorio

project-detail-path = Ruta: { $path }
project-detail-name = Nombre: { $name }

project-overview-tab-label = Visión general
project-overview-header = Visión general
project-overview-detail-name = Nombre: { $name }

project-pcb-header = PCB
project-pcb-designs-header = Diseños

project-placements-tab-label = Ubicaciones
project-placements-header = Ubicaciones de proyecto

project-parts-tab-label = Piezas
project-parts-header = Piezas de proyecto

project-pcb-tab-label = PCB ({ $pcb })

project-process-header = Proceso
project-process-tab-label = Proceso ({ $process })

project-unit-assignments-tab-label = Asignaciones de unidad ({ $pcb })
project-unit-assignments-header = Asignaciones de unidad

phase-toolbar-add-parts-to-loadout = Añadir piezas a la carga.
phase-toolbar-placement-orderings = Ordenaciones de colocación.

phase-placements-header = Ubicaciones de fase

phase-properties-header = Propiedades de fase
phase-properties-footer = { $count } items

phase-reference = Referencia
phase-load-out-source = Fuente de carga
phase-pcb-side = Lado PCB
phase-process = Proceso

project-load-out-header = Carga

table-load-out-column-reference = Reference
table-load-out-column-manufacturer = Fabricante
table-load-out-column-mpn = MPN

pcb-side-top = Superior
pcb-side-bottom = Inferior
pcb-side-both = Ambos

project-explorer-tab-label = Explorador de proyecto

project-explorer-node-root = Raíz
project-explorer-node-parts = Piezas
project-explorer-node-placements = Ubicaciones
project-explorer-node-phases = Fases
project-explorer-node-phase = { $reference } ({ $process})
project-explorer-node-phase-loadout = { $source }
project-explorer-node-unit-assignments = Asignaciones de unidad
project-explorer-node-unit-assignment-assigned = { $name } {$design_name} = {$variant_name}
project-explorer-node-unit-assignment-unassigned = { $name } {$design_name} = (No asignado)
project-explorer-node-pcbs = PCBs
project-explorer-node-pcb = { $name }
project-explorer-node-processes = Procesos
project-explorer-node-process = { $name }

new-pcb-banner = New PCB
form-new-pcb-input-name = Nombre PCB
form-new-pcb-input-name-placeholder = (por ejemplo, el número de referencia del pedido de la fábrica de PCB)
form-new-pcb-input-directory = Directorio
form-new-pcb-input-units = Unidades

pcb-configuration-tab-label = Configuración
pcb-configuration-header = Configuración
pcb-configuration-detail-name = Nombre: { $name }

pcb-panel-tab-label = Panelización de PCB
pcb-panel-tab-panel-orientation-header = Orientación del montaje
pcb-panel-tab-panel-size-header = Tamaño del panel
pcb-panel-tab-panel-edge-rails-header = Raíles de borde
pcb-panel-tab-panel-fiducials-header = Fiduciales
pcb-panel-tab-panel-design-configuration-header = Configuración de diseño
pcb-panel-tab-panel-unit-positions-header = Posiciones de la unidad

pcb-assembly-orientation-rotation = Rotación
pcb-assembly-orientation-flip-tooltip = Terminología:
    Roll = sujetar la pcb por arriba y por abajo y girar 180 grados a lo largo del eje Y (reflejar coordenadas X).
    Pitch = sujetar la pcb por la izquierda y por la derecha y girar 180 grados a lo largo del eje X (reflejar coordenadas Y).
pcb-assembly-orientation-flip-roll = Alabeo (reflejo X)
pcb-assembly-orientation-flip-pitch = Cabeceo (reflejo Y)
pcb-assembly-orientation-flip-none = Ninguno

pcb-explorer-tab-label = Explorador de PCB
pcb-explorer-node-root = { $name }
pcb-explorer-node-configuration = Configuración
pcb-explorer-node-panel = Panelización de PCB
pcb-explorer-node-pcb-view = PCB
pcb-explorer-node-designs = Diseños
pcb-explorer-node-units = Unidad
pcb-explorer-node-units-assignment-assigned = { $pcb_number }: {$design_name}
pcb-explorer-node-units-assignment-unassigned = { $pcb_number }: Sin asignar

pcb-gerber-viewer-tab-label-panel = Panel
pcb-gerber-viewer-tab-label-design = Diseño ({ $index })
pcb-gerber-viewer-layers-window-title = Capas
pcb-gerber-viewer-input-go-to = Ir a

form-configure-pcb-input-units = Units
form-configure-pcb-input-gerber-offset = Despl. de colocación
form-configure-pcb-group-unit-map = Mapa de unidades
form-configure-pcb-input-design-name = Nombre del diseño
form-configure-pcb-input-design-name-placeholder = (p.ej. 'mi_eda_proyecto', para asignaciones de unidades)
form-configure-pcb-input-pcb-unit-range = Rango de unidades
form-configure-pcb-button-panel-gerbers = Gerbers de panel...
form-configure-pcb-button-design-gerbers = Gerbers de diseño...

form-configure-pcb-gerber-offset-help = En el software EDA, se puede especificar un offset al exportar gerbers, por ejemplo (10,5).
    Introduzca aquí desplazamientos negativos para reubicar los gerbers en (0,0), por ejemplo (-10, -5).

form-create-unit-assignment-group-variant-map = Mapa de variantes
form-create-unit-assignment-input-design-name = Nombre del diseño
form-create-unit-assignment-input-design-name-placeholder = Nombre del diseño (por ejemplo, 'mi diseño')
form-create-unit-assignment-input-variant-name = Nombre de la variante
form-create-unit-assignment-input-variant-name-placeholder = Nombre de la variante (por ejemplo, 'default')
form-create-unit-assignment-input-pcb-instance = Instancia PCB
form-create-unit-assignment-input-pcb-instance-placeholder = Un número > 0
form-create-unit-assignment-input-pcb-unit-range = Unidades PCB
form-create-unit-assignment-input-pcb-unit-placeholder = Un número > 0
form-create-unit-assignment-input-placements-filename = Nombre de archivo de las ubicaciones
form-create-unit-assignment-input-placements-filename-placeholder = '<diseño>_<variante>_placements.csv'
form-create-unit-assignment-input-placements-directory = Directorio de ubicaciones

form-phase-placement-orderings-input-orderings = Ordenaciones

form-common-combo-select = Seleccionar...
form-common-combo-none = Ninguno

form-common-choice-pcb-kind = Tipo
form-common-choice-pcb-kind-single = Individual
form-common-choice-pcb-kind-panel = Panel

form-common-choice-pcb-side = Lado PCB
form-common-choice-pcb-side-top = Alto
form-common-choice-pcb-side-bottom = Bajo

form-common-choice-process = Proceso
form-common-choice-phase = Fase

form-common-input-load-out-source = Fuente de carga
form-common-input-phase-reference = Referencia de fase
form-common-input-process-reference = Reference de proceso
form-common-input-x = X
form-common-input-y = Y
form-common-input-top = Arriba
form-common-input-bottom = Abajo
form-common-input-left = Izquierda
form-common-input-right = Derecha
form-common-input-mask-diameter = Máscara ⌀
form-common-input-copper-diameter = Cobre ⌀

form-common-button-assign-selected = Asignar seleccionado
form-common-button-unassign-selected = Desasignar seleccionado
form-common-button-unassign-all = Desasignar todo
form-common-button-unassign-from-range = Desasignar desde rango
form-common-button-unassign-range = Desasignar rango

form-common-button-apply-range = Aplicar rango
form-common-button-apply-all = Aplicar todo

form-common-button-ok = Aceptar
form-common-button-cancel = Cancelar
form-common-button-close = Cerrar
form-common-button-add = Añadir
form-common-button-remove = Quitar
form-common-button-apply = Aplicar
form-common-button-reset = Restablecer
form-common-button-refresh = Actualizar
form-common-button-delete = Borrar

form-option-error-required = * Obligatorio

form-input-error-empty = No puede estar vacío
form-input-error-length = Longitud mínima { $min }
form-input-error-range = Fuera de rango, rango requerido: { $min } - { $max } (inclusive)
form-choice-empty = Elija una opción

form-input-number-require-greater-than-zero = Requiere un número mayor que cero
form-input-number-require-positive-number = Requiere un número
form-file-not-found = Archivo no encontrado

form-input-error-map-incorrect-entry-count = Número de entradas incorrecto.  Requerido: { $required }, Actual: { $actual }
form-input-error-map-unassigned-entries = El mapa contiene entradas sin asignar.

form-input-error-reference-invalid = Referencia inválida
form-input-error-process-reference-invalid = Referencia de proceso inválida
form-input-error-loadout-source-invalid = Fuente de carga inválida

assignment-assigned = Asignado
assignment-unassigned = No asignado

placement-placed = Colocado
placement-pending = Pendiente
placement-skipped = Omitido

placement-place = Lugar
placement-no-place = No-lugar

placement-project-status-used = Utilizado
placement-project-status-unused = No utilizado

sort-mode-feeder-reference = Referencia del alimentador
sort-mode-pcb = Instancia PCB
sort-mode-pcb-unit = Unidad PCB
sort-mode-pcb-unit-xy = Unidad PCB X, luego Y
sort-mode-pcb-unit-yx = Unidad PCB Y, luego X
sort-mode-ref-des = Des. de Ref.

sort-order-ascending = Ascendente
sort-order-descending = Descendente

process-status-pending = Pendiente
process-status-incomplete = Incompleto
process-status-complete = Completo
process-status-abandoned = Abandonado

gerber-file-function-assembly = Montaje
gerber-file-function-component = Componente
gerber-file-function-copper = Cobre
gerber-file-function-legend = Leyenda
gerber-file-function-paste = Pasta
gerber-file-function-profile = Perfil
gerber-file-function-other = Otro
gerber-file-function-solder = Soldar

table-placements-column-index = #
table-placements-column-object-path = Ruta de objeto
table-placements-column-refdes = Des. de Ref.
table-placements-column-place = Coloca
table-placements-column-placed = ¿Colocado?
table-placements-column-manufacturer = Fabricante
table-placements-column-mpn = MPN
table-placements-column-rotation = Rotación
table-placements-column-x = X
table-placements-column-y = Y
table-placements-column-pcb-side = Lado
table-placements-column-phase = Fase
table-placements-column-status = Estado
table-placements-column-ordering = Ordenación

table-parts-column-index = #
table-parts-column-manufacturer = Fabricante
table-parts-column-mpn = MPN
table-parts-column-processes = Procesos
table-parts-column-ref-des-set = Des. de Ref. Conjunto
table-parts-column-quantity = Cantidad

table-designs-column-index = #
table-designs-column-actions = Acciones
table-designs-column-name = Nombre

tabla-gerbers-column-index = #
table-gerbers-column-file = Archivo
table-gerbers-column-pcb-side = Lado PCB
table-gerbers-column-gerber-file-function = Propósito
table-gerbers-column-actions = Acciones

table-gerber-viewer-layers-column-index = #
table-gerber-viewer-layers-column-file = Archivo
table-gerber-viewer-layers-column-pcb-side = Lado PCB
table-gerber-viewer-layers-column-gerber-file-function = Propósito

table-design-assignments-column-pcb-unit = Unidad PCB
table-design-assignments-column-design = Nombre del diseño

table-design-variants-column-design = Nombre del diseño
table-design-variants-column-variant = Nombre de la variante

table-unit-assignments-column-pcb-unit = Unidad PCB
table-unit-assignments-column-design = Nombre del diseño
table-unit-assignments-column-variant = Nombre de la variante

table-design-layout-column-index = #
table-design-layout-column-x-placement-offset = X despl. colocación
table-design-layout-column-y-placement-offset = Y despl. colocación
table-design-layout-column-x-gerber-offset = X gerber despl.
table-design-layout-column-y-gerber-offset = Y gerber despl.
table-design-layout-column-x-origin = X origen
table-design-layout-column-y-origin = Y origen
table-design-layout-column-x-size = X tamaño
table-design-layout-column-y-size = Y tamaño
table-design-layout-column-design-name = Nombre del diseño

table-unit-positions-column-index = #
table-unit-positions-column-x = X
table-unit-positions-column-y = Y
table-unit-positions-column-rotation = Rotación
table-unit-positions-column-design-name = Nombre del diseño

table-fiducials-column-index = #
table-fiducials-column-x = X
table-fiducials-column-y = Y
table-fiducials-column-mask-diameter = Máscara ⌀
table-fiducials-column-copper-diameter = Cobre ⌀
table-fiducials-column-actions = Acciones

common-value-not-available = N/A
common-actions = Acciones

ratio = Relación: { $numerator }: { $denominator }
ratio-error = Ratio: N/A

filter-expression = Buscar...

expanding-header-details = Detalles

#
# errors
#

process-error-name-already-in-use = Attempted to rename a process to a name already in use

#
# egui-data-tables
#

# cell context menu
context-menu-selection-copy = Selección: Copiar
context-menu-selection-cut = Selección: Cortar
context-menu-selection-clear = Selección: Limpiar
context-menu-selection-fill = Selección: Rellenar
context-menu-clipboard-paste = Portapapeles: Pegar
context-menu-clipboard-insert = Portapapeles: Insertar
context-menu-row-duplicate = Fila: Duplicar
context-menu-row-delete = Fila: Eliminar
context-menu-undo = Deshacer
context-menu-redo = Rehacer
# column header context menu
context-menu-hide = Ocultar columna
context-menu-hidden = Columnas ocultas
context-menu-clear-sort = Borrar ordenación
