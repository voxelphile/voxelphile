import { redirect } from "@sveltejs/kit";

export const actions = {
	default: async (event) => {
        event.cookies.delete("jwt", {path: "/"});
        throw redirect(302, "/");
	}
};