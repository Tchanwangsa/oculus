import { List } from "./List";
import { tagSchemaId } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Chart } from "../Chart";
import { Bar } from "../charts/Bar";
import { Distribution } from "../charts/Distribution";
import { Donut } from "../charts/Donut";
import { Histogram } from "../charts/Histogram";
import { Line } from "../charts/Line";
import { Scatter } from "../charts/Scatter";
import { Slider, Value } from "../Slider";
import { Text } from "../Text";
import { Badge } from "./Badge";
import { Button } from "./Button";
import { Callout } from "./Callout";
import { Input } from "./Input";
import { Progress } from "./Progress";
import { Select } from "./Select";
import { Stat } from "./Stat";
import { Table } from "./Table";
import { Toggle } from "./Toggle";

/** What a container holds. Named `Block` in the generated grammar, so each
 *  container's signature says `Block[]` once instead of listing twenty names;
 *  the prompt's Layout notes spell the list out. */
export const BLOCK_NAMES = ["Text", "Bar", "Histogram", "Line", "Scatter", "Donut", "Distribution", "Stat", "Table", "List", "Progress", "Badge", "Callout", "Button", "Toggle", "Select", "Input", "Slider", "Value"];

export const Block = z.union([Chart.ref, Text.ref, Bar.ref, Histogram.ref, Line.ref, Scatter.ref, Donut.ref, Distribution.ref, Stat.ref, Table.ref, List.ref, Progress.ref, Badge.ref, Callout.ref, Button.ref, Toggle.ref, Select.ref, Input.ref, Slider.ref, Value.ref]);
tagSchemaId(Block, "Block");
