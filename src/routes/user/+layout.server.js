
import { error, fail, redirect } from "@sveltejs/kit";
import { fetch_promise } from "../../user-form.js";

/** @type {import('./$types').LayoutServerLoad} */
export async function load(event) {
    if (event.cookies.get("jwt") != undefined) {
        throw redirect(302, "/dashboard");
    }
}